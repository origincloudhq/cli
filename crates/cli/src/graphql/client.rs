use std::time::Duration;

use super::auth::{OAuthAuthenticationError, OAuthClientAuthenticator, TokenProvider};
use http::header;
use serde::de::DeserializeOwned;
use url::Url;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("error: {0}")]
    Other(String),
    #[error("transport: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("authentication: {0}")]
    Authentication(#[from] OAuthAuthenticationError),
    #[error("{}", render_execution_errors(.0))]
    Execution(Vec<graphql_client::Error>),
    #[error("unexpected status: {status}")]
    UnexpectedStatus { status: http::StatusCode },
    /// The server answered without `errors` and without `data`, which the
    /// GraphQL spec does not allow; named so the user sees which call it was
    /// rather than a panic.
    #[error("the server returned no data for {operation}")]
    NoData { operation: &'static str },
}

/// Renders server-side GraphQL errors for a user, not as the Rust `Debug`
/// dump of `graphql_client::Error` (message, `locations`, `path`,
/// `extensions`).
///
/// The server puts a stable `code` and a `correlationId` in `extensions`;
/// both are shown because they are what a user can act on: the code to branch
/// or search on, the correlation id to quote in a support request when the
/// message is deliberately generic.
///
/// The message itself is always shown: masking a real server error behind a
/// generic failure leaves the user nothing to go on.
fn render_execution_errors(errors: &[graphql_client::Error]) -> String {
    errors
        .iter()
        .map(render_execution_error)
        .collect::<Vec<_>>()
        .join("; ")
}

fn render_execution_error(error: &graphql_client::Error) -> String {
    let extension = |key: &str| {
        error
            .extensions
            .as_ref()
            .and_then(|ext| ext.get(key))
            .and_then(|value| value.as_str())
            .map(str::to_owned)
    };

    let mut rendered = match extension("code") {
        Some(code) => format!("{} ({code})", error.message),
        None => error.message.clone(),
    };

    if let Some(correlation_id) = extension("correlationId") {
        rendered.push_str(&format!(" [correlation: {correlation_id}]"));
    }

    rendered
}

/// Loosely-typed mirror of `graphql_client::Response<T>` with `data` left as
/// a raw `serde_json::Value` instead of the caller's strongly-typed shape.
/// A server returning a spec-compliant partial-failure response (a nested
/// non-null field errored, so GraphQL null-propagation makes some ancestor
/// field null, alongside a populated `errors` array) has a `data` that a
/// strongly-typed, all-fields-required shape can't represent - decoding into
/// that shape directly, before even looking at `errors`, turns a real,
/// actionable server error message into a generic decode failure.
#[derive(serde::Deserialize)]
struct RawGraphQLResponse {
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
    errors: Option<Vec<graphql_client::Error>>,
}

fn parse_graphql_response<T: DeserializeOwned>(body: &str) -> Result<Option<T>, Error> {
    let raw: RawGraphQLResponse = serde_json::from_str(body)
        .map_err(|err| Error::Other(format!("failed to decode response body: {err}")))?;

    if let Some(errors) = raw.errors {
        if !errors.is_empty() {
            for (n, error) in errors.iter().enumerate() {
                tracing::warn!("graphql request error: (no. {n}) {error}");
            }
            return Err(Error::Execution(errors));
        }
    }

    raw.data
        .map(serde_json::from_value)
        .transpose()
        .map_err(|err| Error::Other(format!("failed to decode response data: {err}")))
}

/// Whether the answer to a try was lost or the platform says to try again:
/// a transport failure (no response, or none in time), a gateway's
/// 502/503/504, or errors every one of which is marked `retryable`.
fn is_retryable(err: &Error) -> bool {
    match err {
        Error::Transport(err) => !(err.is_builder() || err.is_decode() || err.is_redirect()),
        Error::UnexpectedStatus { status } => matches!(status.as_u16(), 502..=504),
        Error::Execution(errors) => {
            !errors.is_empty()
                && errors.iter().all(|error| {
                    error
                        .extensions
                        .as_ref()
                        .and_then(|ext| ext.get("retryable"))
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                })
        }
        Error::Other(_) | Error::Authentication(_) | Error::NoData { .. } => false,
    }
}

/// A response without `errors` must carry `data`; its absence is an error
/// naming the operation, never a value the caller has to unwrap.
fn require_data<T>(data: Option<T>, operation: &'static str) -> Result<T, Error> {
    data.ok_or(Error::NoData { operation })
}

/// The header a request's idempotency key travels in.
pub const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";

/// How a call is tried again when its answer was lost: a transport failure,
/// a gateway's 502/503/504, or errors the platform marks `retryable`. Every
/// try sends the same request with the same idempotency key, so a mutation
/// that took effect on an earlier try isn't applied again: its first result
/// comes back instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retries {
    /// Tries in all, the first included.
    pub tries: u32,
    /// The wait before the second try; each later wait doubles it.
    pub first_wait: Duration,
}

impl Retries {
    /// A single try.
    pub const NONE: Retries = Retries {
        tries: 1,
        first_wait: Duration::ZERO,
    };
}

impl Default for Retries {
    /// Three tries, one and two seconds apart: longer than a front door
    /// takes to answer that a zone is unavailable.
    fn default() -> Self {
        Retries {
            tries: 3,
            first_wait: Duration::from_secs(1),
        }
    }
}

/// The read-your-writes token a mutation's response carries and a later
/// read presents.
pub const CONSISTENCY_TOKEN_HEADER: &str = "origin-consistency-token";

pub struct GenericGraphQLClient<Auth> {
    authenticator: Auth,
    url: Url,
    http_client: reqwest::Client,
    retries: Retries,
    /// The last consistency token a response gave this client, sent with
    /// every call after it, so the client reads its own writes.
    consistency: std::sync::Mutex<Option<String>>,
}

impl<Auth> GenericGraphQLClient<Auth> {
    /// A client that tries each call once.
    pub fn new(url: Url, authenticator: Auth) -> Self {
        Self {
            authenticator,
            url,
            http_client: Default::default(),
            retries: Retries::NONE,
            consistency: Default::default(),
        }
    }

    pub fn retrying(self, retries: Retries) -> Self {
        Self { retries, ..self }
    }
}

impl<Auth> GenericGraphQLClient<Auth> {
    fn consistency_token(&self) -> Option<String> {
        self.consistency
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl<Auth> GenericGraphQLClient<Auth>
where
    Auth: OAuthClientAuthenticator,
{
    /// Sends the operation and returns its `data`. A response with neither
    /// `data` nor `errors` is [`Error::NoData`]. The call carries an
    /// idempotency key of its own, kept across its tries ([`Retries`]).
    pub async fn call<T: graphql_client::GraphQLQuery>(
        &self,
        variables: T::Variables,
    ) -> Result<T::ResponseData, Error> {
        let request_body = T::build_query(variables);
        let key = uuid::Uuid::new_v4().to_string();
        let mut wait = self.retries.first_wait;
        let mut tried = 1;
        loop {
            match self.try_once::<T>(&request_body, &key).await {
                Err(err) if tried < self.retries.tries && is_retryable(&err) => {
                    tracing::warn!(operation = request_body.operation_name, %err, "trying again in {wait:?}");
                    tokio::time::sleep(wait).await;
                    wait *= 2;
                    tried += 1;
                }
                answered => return answered,
            }
        }
    }

    async fn try_once<T: graphql_client::GraphQLQuery>(
        &self,
        request_body: &graphql_client::QueryBody<T::Variables>,
        key: &str,
    ) -> Result<T::ResponseData, Error> {
        let token = self.authenticator.get_token().await?;
        let operation = request_body.operation_name;
        let mut request = self
            .http_client
            .post(self.url.clone())
            .header(header::AUTHORIZATION, format!("Bearer {}", token.token()))
            .header(IDEMPOTENCY_KEY_HEADER, key);
        if let Some(consistency) = self.consistency_token() {
            request = request.header(CONSISTENCY_TOKEN_HEADER, consistency);
        }
        let response = request.json(request_body).send().await?;

        if !response.status().is_success() {
            return Err(Error::UnexpectedStatus {
                status: response.status(),
            });
        }
        if let Some(issued) = response
            .headers()
            .get(CONSISTENCY_TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
        {
            *self
                .consistency
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(issued.to_string());
        }

        let response_text = response.text().await?;

        require_data(parse_graphql_response(&response_text)?, operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn execution(body: &str) -> Error {
        parse_graphql_response::<serde_json::Value>(body).unwrap_err()
    }

    #[test]
    fn only_a_lost_answer_or_a_retryable_error_is_tried_again() {
        let unavailable = execution(
            r#"{"data":null,"errors":[{"message":"Service unavailable.","extensions":{"code":"SERVICE_UNAVAILABLE","retryable":true}}]}"#,
        );
        let invalid = execution(
            r#"{"data":null,"errors":[{"message":"bad size","extensions":{"code":"BAD_USER_INPUT","retryable":false}}]}"#,
        );
        let unmarked = execution(r#"{"data":null,"errors":[{"message":"no extensions"}]}"#);
        assert!(is_retryable(&unavailable));
        assert!(!is_retryable(&invalid));
        assert!(!is_retryable(&unmarked));
        assert!(is_retryable(&Error::UnexpectedStatus {
            status: http::StatusCode::SERVICE_UNAVAILABLE
        }));
        assert!(!is_retryable(&Error::UnexpectedStatus {
            status: http::StatusCode::UNAUTHORIZED
        }));
    }

    struct FixedToken;

    impl TokenProvider for FixedToken {
        fn token(&self) -> &str {
            "t"
        }
        fn ty(&self) -> &str {
            "Bearer"
        }
    }

    struct FixedAuthenticator;

    impl OAuthClientAuthenticator for FixedAuthenticator {
        async fn get_token(&self) -> Result<impl TokenProvider + Send, OAuthAuthenticationError> {
            Ok(FixedToken)
        }
    }

    struct Ping;

    impl graphql_client::GraphQLQuery for Ping {
        type Variables = ();
        type ResponseData = DummyData;

        fn build_query(variables: ()) -> graphql_client::QueryBody<()> {
            graphql_client::QueryBody {
                variables,
                query: "mutation Ping { value }",
                operation_name: "Ping",
            }
        }
    }

    const UNAVAILABLE: &str =
        "HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
    const ANSWERED: &str = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 23\r\nconnection: close\r\n\r\n{\"data\":{\"value\":\"ok\"}}";
    const CONFLICT: &str = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 80\r\nconnection: close\r\n\r\n{\"data\":null,\"errors\":[{\"message\":\"conflict\",\"extensions\":{\"code\":\"CONFLICT\"}}]}";

    /// A server giving `answers` to its requests in turn, recording each
    /// request's idempotency key.
    async fn server(
        answers: &'static [&'static str],
    ) -> (Url, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        recording(answers, "idempotency-key").await
    }

    /// A server giving `answers` to its requests in turn, recording each
    /// request's `header` (empty when it had none).
    async fn recording(
        answers: &'static [&'static str],
        header: &'static str,
    ) -> (Url, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!(
            "http://{}/graphql",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let keys = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = keys.clone();
        tokio::spawn(async move {
            for answer in answers {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 8192];
                let read = socket.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..read]).to_lowercase();
                let key = request
                    .lines()
                    .find_map(|line| line.strip_prefix(&format!("{header}: ")))
                    .unwrap_or("")
                    .trim()
                    .to_string();
                seen.lock().unwrap().push(key);
                socket.write_all(answer.as_bytes()).await.unwrap();
            }
        });
        (url, keys)
    }

    #[tokio::test]
    async fn a_call_tried_again_sends_the_same_idempotency_key() {
        let (url, keys) = server(&[UNAVAILABLE, ANSWERED]).await;
        let client = GenericGraphQLClient::new(url, FixedAuthenticator).retrying(Retries {
            tries: 3,
            first_wait: Duration::from_millis(10),
        });
        let data = client
            .call::<Ping>(())
            .await
            .expect("answered on the second try");
        assert_eq!(data.value, "ok");
        let keys = keys.lock().unwrap().clone();
        assert_eq!(keys.len(), 2);
        assert!(!keys[0].is_empty(), "the first try carried no key");
        assert_eq!(keys[0], keys[1], "the second try carried another key");
    }

    const ANSWERED_WITH_TOKEN: &str = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\norigin-consistency-token: v1.abc\r\ncontent-length: 23\r\nconnection: close\r\n\r\n{\"data\":{\"value\":\"ok\"}}";

    /// A call after a response that gave a consistency token presents it,
    /// and keeps presenting it.
    #[tokio::test]
    async fn a_call_presents_the_last_consistency_token_it_was_given() {
        let (url, tokens) = recording(
            &[ANSWERED, ANSWERED_WITH_TOKEN, ANSWERED, ANSWERED],
            CONSISTENCY_TOKEN_HEADER,
        )
        .await;
        let client = GenericGraphQLClient::new(url, FixedAuthenticator);
        for _ in 0..4 {
            client.call::<Ping>(()).await.expect("answered");
        }
        assert_eq!(*tokens.lock().unwrap(), ["", "", "v1.abc", "v1.abc"]);
    }

    #[tokio::test]
    async fn a_call_is_tried_no_more_than_its_tries() {
        let (url, keys) = server(&[UNAVAILABLE, UNAVAILABLE, UNAVAILABLE]).await;
        let client = GenericGraphQLClient::new(url, FixedAuthenticator).retrying(Retries {
            tries: 2,
            first_wait: Duration::from_millis(10),
        });
        let failed = client.call::<Ping>(()).await;
        assert!(
            matches!(failed, Err(Error::UnexpectedStatus { .. })),
            "{failed:?}"
        );
        assert_eq!(keys.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn an_answer_that_is_not_retryable_is_not_tried_again() {
        let (url, keys) = server(&[CONFLICT, ANSWERED]).await;
        let client = GenericGraphQLClient::new(url, FixedAuthenticator).retrying(Retries {
            tries: 3,
            first_wait: Duration::from_millis(10),
        });
        let refused = client.call::<Ping>(()).await;
        assert!(matches!(refused, Err(Error::Execution(_))), "{refused:?}");
        assert_eq!(keys.lock().unwrap().len(), 1);
    }

    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct DummyData {
        value: String,
    }

    #[test]
    fn surfaces_the_real_error_message_on_partial_failure() {
        // Spec-compliant partial-failure response: a nested non-null field
        // errored, so GraphQL null-propagation made `pvs` null, alongside a
        // populated `errors` array - exactly what previously got masked
        // behind a generic decode failure.
        let body = r#"{"data":{"pvs":null},"errors":[{"message":"failed to insert snapshot: constraint violation","locations":[{"line":1,"column":66}],"path":["pvs","createSnapshot"]}]}"#;

        match parse_graphql_response::<serde_json::Value>(body) {
            Err(Error::Execution(errors)) => {
                assert_eq!(errors.len(), 1);
                assert!(errors[0].message.contains("constraint violation"));
            }
            other => panic!("expected Error::Execution with the real message, got {other:?}"),
        }
    }

    #[test]
    fn decodes_successful_response_data() {
        let body = r#"{"data":{"value":"ok"}}"#;

        let result = parse_graphql_response::<DummyData>(body).unwrap();

        assert_eq!(
            result,
            Some(DummyData {
                value: "ok".to_string()
            })
        );
    }

    fn parse_required(body: &str) -> Result<DummyData, Error> {
        require_data(parse_graphql_response(body)?, "DummyQuery")
    }

    #[test]
    fn null_data_without_errors_is_an_error_naming_the_operation() {
        match parse_required(r#"{"data":null}"#) {
            Err(Error::NoData { operation }) => assert_eq!(operation, "DummyQuery"),
            other => panic!("expected Error::NoData, got {other:?}"),
        }
    }

    #[test]
    fn absent_data_without_errors_is_an_error_naming_the_operation() {
        match parse_required("{}") {
            Err(Error::NoData { operation }) => assert_eq!(operation, "DummyQuery"),
            other => panic!("expected Error::NoData, got {other:?}"),
        }
    }

    /// What lands in the CLI's `{"code":"failure","description":...}`
    /// envelope.
    #[test]
    fn no_data_renders_the_operation_name() {
        let err = parse_required(r#"{"data":null}"#).expect_err("no data");

        assert_eq!(
            err.to_string(),
            "the server returned no data for DummyQuery"
        );
    }

    /// `errors` still wins over a missing `data`: the server's message is
    /// what the user needs.
    #[test]
    fn errors_take_precedence_over_missing_data() {
        let body = r#"{"data":null,"errors":[{"message":"denied"}]}"#;

        assert!(matches!(parse_required(body), Err(Error::Execution(_))));
    }

    /// The CLI-facing rendering must not be Rust `Debug` output, while
    /// still showing the server's message.
    #[test]
    fn renders_code_and_correlation_id_without_debug_output() {
        let body = r#"{"data":null,"errors":[{"message":"Internal server error.","locations":[{"line":1,"column":66}],"path":["pvs","createSnapshot"],"extensions":{"code":"INTERNAL_SERVER_ERROR","retryable":false,"correlationId":"4bf92f3577b34da6a3ce929d0e0e4736"}}]}"#;

        let err = parse_graphql_response::<serde_json::Value>(body)
            .expect_err("errors array is populated");
        let rendered = err.to_string();

        assert!(rendered.contains("Internal server error."));
        assert!(rendered.contains("INTERNAL_SERVER_ERROR"));
        assert!(rendered.contains("4bf92f3577b34da6a3ce929d0e0e4736"));
        // Not the `Debug` struct dump this used to be.
        assert!(!rendered.contains("locations"));
        assert!(!rendered.contains("Error {"));
        assert!(!rendered.contains("Some("));
    }

    /// A server that has not yet been migrated sends no extensions; the
    /// message alone must still come through rather than being swallowed.
    #[test]
    fn falls_back_to_the_bare_message_without_extensions() {
        let body = r#"{"data":null,"errors":[{"message":"invalid input on field sizeInGib: must be at least 1"}]}"#;

        let err = parse_graphql_response::<serde_json::Value>(body)
            .expect_err("errors array is populated");

        assert_eq!(
            err.to_string(),
            "invalid input on field sizeInGib: must be at least 1"
        );
    }

    #[test]
    fn joins_multiple_errors_readably() {
        let body = r#"{"data":null,"errors":[{"message":"first","extensions":{"code":"BAD_REQUEST"}},{"message":"second","extensions":{"code":"NOT_FOUND"}}]}"#;

        let err = parse_graphql_response::<serde_json::Value>(body)
            .expect_err("errors array is populated");

        assert_eq!(err.to_string(), "first (BAD_REQUEST); second (NOT_FOUND)");
    }
}
