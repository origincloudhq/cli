//! Authorization Code with PKCE and a loopback redirect (RFC 7636, RFC 8252).
//!
//! The redirect target is a one-shot HTTP listener on `127.0.0.1` with an
//! OS-assigned port. It serves until one request reaches the callback path,
//! answers the browser, and is dropped with everything it accepted. Deciding
//! what a callback means ([`evaluate_callback`]) is pure and tested apart
//! from the socket.

use crate::endpoints::OidcEndpoints;
use crate::error::LoginError;
use crate::pkce::{random_state, Pkce, CODE_CHALLENGE_METHOD};
use crate::progress::LoginProgress;
use crate::token::{post_form, TokenResponse};
use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use url::Url;

const CALLBACK_PATH: &str = "/callback";

/// Largest request head read from the browser. A callback is one request
/// line with a code and a state; anything larger is not one.
const MAX_REQUEST_HEAD_BYTES: usize = 8 * 1024;

/// How long one accepted connection may take to send its request. Browsers
/// open speculative connections that never send anything, so each connection
/// is served on its own task and abandoned after this.
const CONNECTION_READ_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct BrowserLoginOptions {
    /// How long to wait for the browser to come back before giving up.
    pub timeout: Duration,
    /// Pre-fills the login page's username (`login_hint`). The person can
    /// still change it there.
    pub login_hint: Option<String>,
    /// Whether to launch the system browser. The authorization URL is shown
    /// through [`LoginProgress`] either way.
    pub open_browser: bool,
}

impl Default for BrowserLoginOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5 * 60),
            login_hint: None,
            open_browser: true,
        }
    }
}

pub(crate) async fn run(
    http: &reqwest::Client,
    endpoints: &OidcEndpoints,
    client_id: &str,
    options: &BrowserLoginOptions,
    progress: &dyn LoginProgress,
) -> Result<TokenResponse, LoginError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(LoginError::Listener)?;
    let port = listener.local_addr().map_err(LoginError::Listener)?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");

    let pkce = Pkce::generate();
    let state = random_state();
    let url = authorization_url(
        &endpoints.authorization,
        client_id,
        &redirect_uri,
        &pkce,
        &state,
        options.login_hint.as_deref(),
    );

    let opened = options.open_browser && launch_browser(&url);
    progress.show_authorization_url(&url, opened);

    let code = tokio::time::timeout(options.timeout, await_callback(listener, state))
        .await
        .map_err(|_| LoginError::TimedOut(options.timeout))??;

    post_form(
        http,
        &endpoints.token,
        &[
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("client_id", client_id),
            ("code_verifier", pkce.verifier()),
        ],
    )
    .await?
    .map_err(LoginError::from)
}

fn authorization_url(
    authorization_endpoint: &Url,
    client_id: &str,
    redirect_uri: &str,
    pkce: &Pkce,
    state: &str,
    login_hint: Option<&str>,
) -> Url {
    let mut url = authorization_endpoint.clone();
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", "openid")
            .append_pair("state", state)
            .append_pair("code_challenge", pkce.challenge())
            .append_pair("code_challenge_method", CODE_CHALLENGE_METHOD);
        if let Some(hint) = login_hint.filter(|h| !h.trim().is_empty()) {
            query.append_pair("login_hint", hint.trim());
        }
    }
    url
}

/// Accepts connections until one carries a callback, and returns what it
/// decided. Returning drops the listener, so nothing listens afterwards;
/// connection tasks still waiting on a silent socket end at their timeout.
async fn await_callback(listener: TcpListener, state: String) -> Result<String, LoginError> {
    let (verdicts, mut received) = mpsc::channel(1);
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(LoginError::Listener)?;
                tokio::spawn(serve_connection(stream, state.clone(), verdicts.clone()));
            }
            Some(verdict) = received.recv() => return verdict,
        }
    }
}

async fn serve_connection(
    mut stream: TcpStream,
    state: String,
    verdicts: mpsc::Sender<Result<String, LoginError>>,
) {
    let head =
        match tokio::time::timeout(CONNECTION_READ_TIMEOUT, read_request_head(&mut stream)).await {
            Ok(Ok(head)) => head,
            // A silent or broken connection is not a login answer.
            _ => return,
        };
    let Some(query) = callback_query(&head) else {
        // Anything else the browser asks for, such as a favicon.
        let _ = stream.write_all(NOT_FOUND_RESPONSE.as_bytes()).await;
        return;
    };

    let verdict = evaluate_callback(query, &state);
    let page = if verdict.is_ok() {
        SUCCESS_RESPONSE
    } else {
        FAILURE_RESPONSE
    };
    // The page is written before the verdict is handed over, so the browser
    // has its answer before the command exits.
    let _ = stream.write_all(page.as_bytes()).await;
    let _ = stream.shutdown().await;
    let _ = verdicts.send(verdict).await;
}

async fn read_request_head(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        if head.len() >= MAX_REQUEST_HEAD_BYTES {
            break;
        }
        let read = stream.read(&mut buf).await?;
        if read == 0 {
            break;
        }
        head.extend_from_slice(&buf[..read]);
    }
    Ok(String::from_utf8_lossy(&head).into_owned())
}

/// The query string of a `GET` to the callback path, or `None` for any other
/// request. A callback with no query at all yields an empty one, which
/// [`evaluate_callback`] then rejects.
fn callback_query(request_head: &str) -> Option<&str> {
    let request_line = request_head.lines().next()?;
    let mut parts = request_line.split(' ');
    if parts.next()? != "GET" {
        return None;
    }
    let target = parts.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    (path == CALLBACK_PATH).then_some(query)
}

/// What a callback's query means for this login: the authorization code to
/// redeem, or why there is none.
///
/// `state` is checked first and on every response, including errors: a
/// response carrying some other state belongs to some other attempt, and
/// nothing in it, not even its error, is about this one.
fn evaluate_callback(query: &str, expected_state: &str) -> Result<String, LoginError> {
    let mut state = None;
    let mut code = None;
    let mut error = None;
    let mut error_description = None;
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "state" => state = Some(value.into_owned()),
            "code" => code = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            "error_description" => error_description = Some(value.into_owned()),
            _ => {}
        }
    }

    if state.as_deref() != Some(expected_state) {
        return Err(LoginError::StateMismatch);
    }
    if let Some(error) = error {
        return Err(LoginError::AuthorizationFailed {
            error,
            description: error_description,
        });
    }
    code.filter(|c| !c.is_empty())
        .ok_or_else(|| LoginError::MalformedCallback("it carries no authorization code".into()))
}

/// Launches the system browser on `url`, returning whether a launcher could
/// be started. On Linux without a display there is nothing to launch into,
/// so it is not tried.
fn launch_browser(url: &Url) -> bool {
    #[cfg(target_os = "macos")]
    let launcher = "open";
    #[cfg(not(target_os = "macos"))]
    let launcher = {
        // Set-but-empty is as good as unset: `DISPLAY= origin id login`.
        let has_display = |name| std::env::var_os(name).is_some_and(|v| !v.is_empty());
        if !has_display("DISPLAY") && !has_display("WAYLAND_DISPLAY") {
            return false;
        }
        "xdg-open"
    };

    let spawned = std::process::Command::new(launcher)
        .arg(url.as_str())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    match spawned {
        Ok(mut child) => {
            // Reaped off the async runtime: a launcher may outlive the
            // browser it starts, and nothing waits on its result.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(err) => {
            tracing::debug!(%err, launcher, "could not launch a browser");
            false
        }
    }
}

const SUCCESS_RESPONSE: &str = "HTTP/1.1 200 OK\r\n\
Content-Type: text/html; charset=utf-8\r\n\
Cache-Control: no-store\r\n\
Connection: close\r\n\r\n\
<!doctype html><title>Logged in</title>\
<p>You are logged in. You can close this tab and return to the terminal.</p>";

/// Deliberately says nothing about why: the reason is printed in the
/// terminal, and echoing the callback's parameters into a page would be
/// rendering attacker-chosen text.
const FAILURE_RESPONSE: &str = "HTTP/1.1 400 Bad Request\r\n\
Content-Type: text/html; charset=utf-8\r\n\
Cache-Control: no-store\r\n\
Connection: close\r\n\r\n\
<!doctype html><title>Login failed</title>\
<p>The login did not complete. Return to the terminal for details.</p>";

const NOT_FOUND_RESPONSE: &str = "HTTP/1.1 404 Not Found\r\n\
Content-Length: 0\r\n\
Connection: close\r\n\r\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_authorization_url_carries_an_s256_challenge_and_the_state() {
        let endpoint = Url::parse("https://idp/realms/r/protocol/openid-connect/auth").unwrap();
        let pkce = Pkce::generate();
        let url = authorization_url(
            &endpoint,
            "origin-cli",
            "http://127.0.0.1:4711/callback",
            &pkce,
            "the-state",
            Some(" ada "),
        );
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();

        assert_eq!(pairs["response_type"], "code");
        assert_eq!(pairs["client_id"], "origin-cli");
        assert_eq!(pairs["redirect_uri"], "http://127.0.0.1:4711/callback");
        assert_eq!(pairs["state"], "the-state");
        assert_eq!(pairs["code_challenge"], pkce.challenge());
        assert_eq!(pairs["code_challenge_method"], "S256");
        assert_eq!(pairs["login_hint"], "ada");
        assert!(!url.as_str().contains(pkce.verifier()));
    }

    #[test]
    fn a_blank_login_hint_is_left_out() {
        let endpoint = Url::parse("https://idp/auth").unwrap();
        let url = authorization_url(&endpoint, "c", "r", &Pkce::generate(), "s", Some("  "));
        assert!(!url.query_pairs().any(|(k, _)| k == "login_hint"));
    }

    #[test]
    fn a_callback_with_the_right_state_yields_its_code() {
        assert_eq!(
            evaluate_callback("state=s1&session_state=x&code=c%2F1", "s1").unwrap(),
            "c/1"
        );
    }

    #[test]
    fn a_callback_with_another_state_is_refused_even_with_a_code() {
        assert!(matches!(
            evaluate_callback("state=other&code=c", "s1"),
            Err(LoginError::StateMismatch)
        ));
    }

    #[test]
    fn a_callback_without_a_state_is_refused() {
        assert!(matches!(
            evaluate_callback("code=c", "s1"),
            Err(LoginError::StateMismatch)
        ));
    }

    /// An error response for some other attempt is still that attempt's.
    #[test]
    fn an_error_callback_with_another_state_is_a_state_mismatch() {
        assert!(matches!(
            evaluate_callback("state=other&error=access_denied", "s1"),
            Err(LoginError::StateMismatch)
        ));
    }

    #[test]
    fn an_error_callback_fails_naming_the_error_and_description() {
        let err = evaluate_callback(
            "state=s1&error=access_denied&error_description=User+cancelled",
            "s1",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            LoginError::AuthorizationFailed { ref error, ref description }
                if error == "access_denied" && description.as_deref() == Some("User cancelled")
        ));
        assert!(err.to_string().contains("User cancelled"));
    }

    #[test]
    fn a_callback_without_a_code_is_malformed() {
        assert!(matches!(
            evaluate_callback("state=s1", "s1"),
            Err(LoginError::MalformedCallback(_))
        ));
        assert!(matches!(
            evaluate_callback("state=s1&code=", "s1"),
            Err(LoginError::MalformedCallback(_))
        ));
    }

    #[test]
    fn only_a_get_to_the_callback_path_is_a_callback() {
        assert_eq!(
            callback_query("GET /callback?code=c&state=s HTTP/1.1\r\nHost: x\r\n\r\n"),
            Some("code=c&state=s")
        );
        assert_eq!(callback_query("GET /callback HTTP/1.1\r\n\r\n"), Some(""));
        assert_eq!(callback_query("GET /favicon.ico HTTP/1.1\r\n\r\n"), None);
        assert_eq!(
            callback_query("GET /callbackx?code=c HTTP/1.1\r\n\r\n"),
            None
        );
        assert_eq!(
            callback_query("POST /callback?code=c HTTP/1.1\r\n\r\n"),
            None
        );
        assert_eq!(callback_query(""), None);
    }
}
