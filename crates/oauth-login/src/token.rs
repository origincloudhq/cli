use crate::error::LoginError;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use url::Url;

/// A successful token-endpoint response, as Keycloak sends it.
///
/// `refresh_expires_in` is Keycloak's own addition to RFC 6749's response; a
/// tool needs it to know when a stored session has to be logged into again.
#[derive(Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: i64,
    pub refresh_token: String,
    pub refresh_expires_in: i64,
    pub token_type: String,
}

/// Hand-written so that neither token can be printed.
impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &"*****")
            .field("expires_in", &self.expires_in)
            .field("refresh_token", &"*****")
            .field("refresh_expires_in", &self.refresh_expires_in)
            .field("token_type", &self.token_type)
            .finish()
    }
}

/// An OAuth error response body (RFC 6749 §5.2, RFC 8628 §3.5).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct OAuthErrorBody {
    pub(crate) error: String,
    #[serde(default)]
    pub(crate) error_description: Option<String>,
}

impl From<OAuthErrorBody> for LoginError {
    fn from(body: OAuthErrorBody) -> Self {
        LoginError::Refused {
            error: body.error,
            description: body.error_description,
        }
    }
}

/// What an OAuth endpoint answered: the expected body, or an OAuth error the
/// caller interprets. Device polling treats some errors as "keep going", so
/// an error reply is not a [`LoginError`] yet.
pub(crate) type Reply<T> = Result<T, OAuthErrorBody>;

/// Longest slice of an unparseable body quoted in an error, so an HTML error
/// page from a proxy does not flood the terminal.
const QUOTED_BODY_LIMIT: usize = 300;

/// POSTs a form to an OAuth endpoint and parses the answer as `T` on success
/// or as an OAuth error otherwise. Anything else is
/// [`LoginError::UnexpectedResponse`].
pub(crate) async fn post_form<T: DeserializeOwned>(
    http: &reqwest::Client,
    endpoint: &Url,
    params: &[(&str, &str)],
) -> Result<Reply<T>, LoginError> {
    let response = http.post(endpoint.clone()).form(params).send().await?;
    let status = response.status();
    let body = response.text().await?;
    parse_reply(status.as_u16(), &body)
}

fn parse_reply<T: DeserializeOwned>(status: u16, body: &str) -> Result<Reply<T>, LoginError> {
    if (200..300).contains(&status) {
        if let Ok(value) = serde_json::from_str::<T>(body) {
            return Ok(Ok(value));
        }
    } else if let Ok(error) = serde_json::from_str::<OAuthErrorBody>(body) {
        return Ok(Err(error));
    }
    Err(LoginError::UnexpectedResponse {
        status,
        body: body.chars().take(QUOTED_BODY_LIMIT).collect(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn debug_hides_the_tokens() {
        let token = super::TokenResponse {
            access_token: "secret-access".to_owned(),
            expires_in: 300,
            refresh_token: "secret-refresh".to_owned(),
            refresh_expires_in: 1800,
            token_type: "Bearer".to_owned(),
        };
        let printed = format!("{token:?}");
        assert!(!printed.contains("secret"), "{printed}");
    }

    use super::*;

    const TOKEN: &str = r#"{"access_token":"a","expires_in":300,"refresh_token":"r",
        "refresh_expires_in":1800,"token_type":"Bearer","scope":"openid"}"#;

    #[test]
    fn a_success_body_is_a_token() {
        let reply = parse_reply::<TokenResponse>(200, TOKEN).unwrap();
        assert!(matches!(reply, Ok(t) if t.access_token == "a"));
    }

    #[test]
    fn an_oauth_error_body_is_an_error_reply_with_its_description() {
        let reply = parse_reply::<TokenResponse>(
            400,
            r#"{"error":"invalid_grant","error_description":"Code not valid"}"#,
        )
        .unwrap();
        let Err(body) = reply else {
            panic!("expected an error reply");
        };
        assert_eq!(body.error, "invalid_grant");
        assert_eq!(body.error_description.as_deref(), Some("Code not valid"));
    }

    #[test]
    fn an_error_without_a_description_still_parses() {
        let reply =
            parse_reply::<TokenResponse>(400, r#"{"error":"authorization_pending"}"#).unwrap();
        assert!(matches!(reply, Err(b) if b.error == "authorization_pending"));
    }

    #[test]
    fn a_body_that_is_neither_is_unexpected_and_quoted_truncated() {
        let html = "<html>".repeat(200);
        let err = parse_reply::<TokenResponse>(502, &html).unwrap_err();
        let LoginError::UnexpectedResponse { status, body } = err else {
            panic!("expected UnexpectedResponse");
        };
        assert_eq!(status, 502);
        assert_eq!(body.chars().count(), QUOTED_BODY_LIMIT);
    }

    /// A 200 that is not a token response is not silently an empty login.
    #[test]
    fn a_success_status_with_a_non_token_body_is_unexpected() {
        assert!(matches!(
            parse_reply::<TokenResponse>(200, r#"{"error":"x"}"#),
            Err(LoginError::UnexpectedResponse { status: 200, .. })
        ));
    }
}
