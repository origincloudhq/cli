//! Both logins and the refresh end to end, against a stub identity provider
//! on loopback: the real requests, the real callback listener, the real
//! token exchange. Only the identity provider and the browser are stand-ins,
//! and device polling sleeps through an injected function that records
//! instead of waiting.

use crate::browser::{self, BrowserLoginOptions};
use crate::device::{self, DeviceCodePrompt};
use crate::endpoints::OidcEndpoints;
use crate::error::LoginError;
use crate::progress::LoginProgress;
use crate::NativeLoginClient;
use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use url::Url;

const CLIENT_ID: &str = "test-cli";

const TOKEN_BODY: &str = r#"{"access_token":"access-1","expires_in":300,
    "refresh_token":"refresh-1","refresh_expires_in":1800,"token_type":"Bearer"}"#;

type Scripted = (StatusCode, &'static str);

#[derive(Clone, Default)]
struct Stub {
    device_reply: Arc<Mutex<Option<Scripted>>>,
    token_replies: Arc<Mutex<VecDeque<Scripted>>>,
    device_requests: Arc<Mutex<Vec<HashMap<String, String>>>>,
    token_requests: Arc<Mutex<Vec<HashMap<String, String>>>>,
}

impl Stub {
    fn reply_to_device(self, reply: Scripted) -> Self {
        *self.device_reply.lock().unwrap() = Some(reply);
        self
    }

    fn reply_to_token(self, replies: &[Scripted]) -> Self {
        self.token_replies
            .lock()
            .unwrap()
            .extend(replies.iter().copied());
        self
    }

    fn token_requests(&self) -> Vec<HashMap<String, String>> {
        self.token_requests.lock().unwrap().clone()
    }

    async fn serve(&self) -> OidcEndpoints {
        let app = Router::new()
            .route(
                "/realms/r/protocol/openid-connect/auth/device",
                post(device_endpoint),
            )
            .route(
                "/realms/r/protocol/openid-connect/token",
                post(token_endpoint),
            )
            .with_state(self.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        OidcEndpoints::keycloak_realm(&base, "r").unwrap()
    }
}

async fn device_endpoint(
    State(stub): State<Stub>,
    Form(form): Form<HashMap<String, String>>,
) -> (StatusCode, &'static str) {
    stub.device_requests.lock().unwrap().push(form);
    stub.device_reply
        .lock()
        .unwrap()
        .expect("the test scripted no device reply")
}

async fn token_endpoint(
    State(stub): State<Stub>,
    Form(form): Form<HashMap<String, String>>,
) -> (StatusCode, &'static str) {
    stub.token_requests.lock().unwrap().push(form);
    stub.token_replies
        .lock()
        .unwrap()
        .pop_front()
        .expect("the client polled more often than the test scripted")
}

const DEVICE_AUTHORIZATION: Scripted = (
    StatusCode::OK,
    r#"{"device_code":"device-1","user_code":"WXYZ-1234",
        "verification_uri":"https://idp/device",
        "verification_uri_complete":"https://idp/device?user_code=WXYZ-1234",
        "expires_in":600,"interval":3}"#,
);
const PENDING: Scripted = (
    StatusCode::BAD_REQUEST,
    r#"{"error":"authorization_pending"}"#,
);
const SLOW_DOWN: Scripted = (StatusCode::BAD_REQUEST, r#"{"error":"slow_down"}"#);
const TOKEN: Scripted = (StatusCode::OK, TOKEN_BODY);

/// Records what a login showed. The browser login's URL is also forwarded
/// to the test's stand-in browser.
#[derive(Default)]
struct Recorder {
    urls: Mutex<Vec<(Url, bool)>>,
    device_prompts: Mutex<Vec<DeviceCodePrompt>>,
    browser: Option<mpsc::UnboundedSender<Url>>,
}

impl LoginProgress for Recorder {
    fn show_authorization_url(&self, url: &Url, browser_opened: bool) {
        self.urls
            .lock()
            .unwrap()
            .push((url.clone(), browser_opened));
        if let Some(browser) = &self.browser {
            browser.send(url.clone()).unwrap();
        }
    }

    fn show_device_code(&self, prompt: &DeviceCodePrompt) {
        self.device_prompts.lock().unwrap().push(prompt.clone());
    }
}

async fn run_device_login(
    endpoints: &OidcEndpoints,
    recorder: &Recorder,
) -> (Result<crate::TokenResponse, LoginError>, Vec<Duration>) {
    let sleeps = Arc::new(Mutex::new(Vec::new()));
    let recorded = sleeps.clone();
    let outcome = device::run(
        &reqwest::Client::default(),
        endpoints,
        CLIENT_ID,
        recorder,
        move |wait| {
            recorded.lock().unwrap().push(wait);
            std::future::ready(())
        },
    )
    .await;
    let sleeps = sleeps.lock().unwrap().clone();
    (outcome, sleeps)
}

fn secs(values: &[u64]) -> Vec<Duration> {
    values.iter().map(|s| Duration::from_secs(*s)).collect()
}

#[tokio::test]
async fn device_login_polls_through_pending_and_slow_down_to_a_token() {
    let stub = Stub::default()
        .reply_to_device(DEVICE_AUTHORIZATION)
        .reply_to_token(&[PENDING, SLOW_DOWN, PENDING, TOKEN]);
    let endpoints = stub.serve().await;
    let recorder = Recorder::default();

    let (outcome, sleeps) = run_device_login(&endpoints, &recorder).await;

    assert_eq!(outcome.unwrap().access_token, "access-1");
    // The server's interval of 3, then 3 + 5 from the slow_down onwards.
    assert_eq!(sleeps, secs(&[3, 3, 8, 8]));

    let prompt = recorder.device_prompts.lock().unwrap()[0].clone();
    assert_eq!(prompt.user_code, "WXYZ-1234");
    assert_eq!(prompt.verification_uri, "https://idp/device");
    assert_eq!(
        prompt.verification_uri_complete.as_deref(),
        Some("https://idp/device?user_code=WXYZ-1234")
    );

    let device_request = stub.device_requests.lock().unwrap()[0].clone();
    assert_eq!(device_request["client_id"], CLIENT_ID);
    // A client that requires PKCE refuses a device request without it.
    assert_eq!(device_request["code_challenge_method"], "S256");
    for request in stub.token_requests() {
        assert_eq!(
            request["grant_type"],
            "urn:ietf:params:oauth:grant-type:device_code"
        );
        assert_eq!(request["device_code"], "device-1");
        assert_eq!(request["client_id"], CLIENT_ID);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(request["code_verifier"].as_bytes()));
        assert_eq!(
            challenge, device_request["code_challenge"],
            "every poll proves the challenge the device request sent"
        );
    }
}

#[tokio::test]
async fn device_login_fails_cleanly_when_the_code_expires() {
    let stub = Stub::default()
        .reply_to_device(DEVICE_AUTHORIZATION)
        .reply_to_token(&[
            PENDING,
            (StatusCode::BAD_REQUEST, r#"{"error":"expired_token"}"#),
        ]);
    let endpoints = stub.serve().await;

    let (outcome, _) = run_device_login(&endpoints, &Recorder::default()).await;

    assert!(matches!(outcome, Err(LoginError::DeviceCodeExpired)));
    assert_eq!(stub.token_requests().len(), 2);
}

#[tokio::test]
async fn device_login_fails_cleanly_when_denied() {
    let stub = Stub::default()
        .reply_to_device(DEVICE_AUTHORIZATION)
        .reply_to_token(&[(StatusCode::BAD_REQUEST, r#"{"error":"access_denied"}"#)]);
    let endpoints = stub.serve().await;

    let (outcome, _) = run_device_login(&endpoints, &Recorder::default()).await;

    assert!(matches!(outcome, Err(LoginError::AccessDenied)));
}

/// A client with the device grant switched off is refused at the first
/// request, before anything is shown or polled.
#[tokio::test]
async fn device_login_refused_up_front_shows_nothing_and_never_polls() {
    let stub = Stub::default().reply_to_device((
        StatusCode::BAD_REQUEST,
        r#"{"error":"unauthorized_client","error_description":"Client is not allowed to initiate OAuth 2.0 Device Authorization Grant"}"#,
    ));
    let endpoints = stub.serve().await;
    let recorder = Recorder::default();

    let (outcome, sleeps) = run_device_login(&endpoints, &recorder).await;

    let err = outcome.unwrap_err();
    assert!(matches!(err, LoginError::Refused { ref error, .. } if error == "unauthorized_client"));
    assert!(err.to_string().contains("Device Authorization Grant"));
    assert!(recorder.device_prompts.lock().unwrap().is_empty());
    assert!(sleeps.is_empty());
    assert!(stub.token_requests().is_empty());
}

fn browser_options(timeout: Duration) -> BrowserLoginOptions {
    BrowserLoginOptions {
        timeout,
        login_hint: None,
        open_browser: false,
    }
}

/// Starts a browser login and hands back the authorization URL the
/// stand-in browser would have opened.
fn start_browser_login(
    endpoints: OidcEndpoints,
    timeout: Duration,
) -> (
    tokio::task::JoinHandle<Result<crate::TokenResponse, LoginError>>,
    mpsc::UnboundedReceiver<Url>,
) {
    let (browser, opened) = mpsc::unbounded_channel();
    let login = tokio::spawn(async move {
        let recorder = Recorder {
            browser: Some(browser),
            ..Recorder::default()
        };
        browser::run(
            &reqwest::Client::default(),
            &endpoints,
            CLIENT_ID,
            &browser_options(timeout),
            &recorder,
        )
        .await
    });
    (login, opened)
}

fn query_param(url: &Url, key: &str) -> String {
    url.query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| panic!("{key} missing from {url}"))
}

#[tokio::test]
async fn browser_login_redeems_the_code_with_the_matching_pkce_verifier() {
    let stub = Stub::default().reply_to_token(&[TOKEN]);
    let (login, mut opened) = start_browser_login(stub.serve().await, Duration::from_secs(30));

    let url = opened.recv().await.unwrap();
    let redirect_uri = Url::parse(&query_param(&url, "redirect_uri")).unwrap();
    assert_eq!(redirect_uri.host_str(), Some("127.0.0.1"));
    assert_eq!(redirect_uri.path(), "/callback");
    let port = redirect_uri.port().unwrap();

    // A speculative connection that never sends anything must not hold up
    // the real callback behind it.
    let _idle = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();

    let browser = reqwest::Client::default();
    let favicon = browser
        .get(format!("http://127.0.0.1:{port}/favicon.ico"))
        .send()
        .await
        .unwrap();
    assert_eq!(favicon.status(), 404);

    let mut callback = redirect_uri.clone();
    callback
        .query_pairs_mut()
        .append_pair("state", &query_param(&url, "state"))
        .append_pair("code", "the-code");
    let page = browser.get(callback).send().await.unwrap();
    assert_eq!(page.status(), 200);
    assert!(page.text().await.unwrap().contains("close this tab"));

    assert_eq!(login.await.unwrap().unwrap().access_token, "access-1");

    let exchange = &stub.token_requests()[0];
    assert_eq!(exchange["grant_type"], "authorization_code");
    assert_eq!(exchange["code"], "the-code");
    assert_eq!(exchange["client_id"], CLIENT_ID);
    assert_eq!(exchange["redirect_uri"], redirect_uri.as_str());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(exchange["code_verifier"].as_bytes()));
    assert_eq!(challenge, query_param(&url, "code_challenge"));
}

#[tokio::test]
async fn browser_login_with_a_mismatched_state_fails_and_stops_listening() {
    let stub = Stub::default();
    let (login, mut opened) = start_browser_login(stub.serve().await, Duration::from_secs(30));

    let url = opened.recv().await.unwrap();
    let mut callback = Url::parse(&query_param(&url, "redirect_uri")).unwrap();
    let port = callback.port().unwrap();
    callback
        .query_pairs_mut()
        .append_pair("state", "not-this-login")
        .append_pair("code", "the-code");
    let page = reqwest::get(callback).await.unwrap();
    assert_eq!(page.status(), 400);

    assert!(matches!(
        login.await.unwrap(),
        Err(LoginError::StateMismatch)
    ));
    assert!(
        stub.token_requests().is_empty(),
        "a code from another attempt must never be redeemed"
    );
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_err(),
        "the callback listener must be gone once the login has ended"
    );
}

#[tokio::test]
async fn browser_login_reports_an_error_the_browser_came_back_with() {
    let (login, mut opened) =
        start_browser_login(Stub::default().serve().await, Duration::from_secs(30));

    let url = opened.recv().await.unwrap();
    let mut callback = Url::parse(&query_param(&url, "redirect_uri")).unwrap();
    callback
        .query_pairs_mut()
        .append_pair("state", &query_param(&url, "state"))
        .append_pair("error", "access_denied")
        .append_pair("error_description", "User cancelled");
    reqwest::get(callback).await.unwrap();

    let err = login.await.unwrap().unwrap_err();
    assert!(
        matches!(err, LoginError::AuthorizationFailed { ref error, .. } if error == "access_denied")
    );
}

#[tokio::test]
async fn browser_login_times_out_when_the_browser_never_comes_back() {
    let (login, mut opened) =
        start_browser_login(Stub::default().serve().await, Duration::from_millis(200));

    let url = opened.recv().await.unwrap();
    let port = Url::parse(&query_param(&url, "redirect_uri"))
        .unwrap()
        .port()
        .unwrap();
    // A connection that sends half a request and stalls is no answer either.
    let mut stalled = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    stalled.write_all(b"GET /callback?co").await.unwrap();

    assert!(matches!(
        login.await.unwrap(),
        Err(LoginError::TimedOut(t)) if t == Duration::from_millis(200)
    ));
}

#[tokio::test]
async fn refresh_sends_the_refresh_token_for_this_client() {
    let stub = Stub::default().reply_to_token(&[TOKEN]);
    let client = NativeLoginClient::new(stub.serve().await, CLIENT_ID);

    let token = client.refresh("refresh-0").await.unwrap();

    assert_eq!(token.refresh_token, "refresh-1");
    let request = &stub.token_requests()[0];
    assert_eq!(request["grant_type"], "refresh_token");
    assert_eq!(request["refresh_token"], "refresh-0");
    assert_eq!(request["client_id"], CLIENT_ID);
}

#[tokio::test]
async fn an_expired_refresh_token_is_refused_with_the_server_s_reason() {
    let stub = Stub::default().reply_to_token(&[(
        StatusCode::BAD_REQUEST,
        r#"{"error":"invalid_grant","error_description":"Token is not active"}"#,
    )]);
    let client = NativeLoginClient::new(stub.serve().await, CLIENT_ID);

    let err = client.refresh("refresh-0").await.unwrap_err();

    assert!(err.to_string().contains("Token is not active"));
}
