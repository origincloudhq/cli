//! The Device Authorization Grant (RFC 8628).
//!
//! What to do with each token-endpoint answer is decided by [`DevicePoll`], a
//! pure state machine, so every RFC 8628 §3.5 case is testable without a
//! server or a clock. [`run`] only sends the requests and sleeps.

use crate::endpoints::OidcEndpoints;
use crate::error::LoginError;
use crate::pkce::{Pkce, CODE_CHALLENGE_METHOD};
use crate::progress::LoginProgress;
use crate::token::{post_form, Reply, TokenResponse};
use serde::Deserialize;
use std::future::Future;
use std::time::Duration;

pub(crate) const DEVICE_CODE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// RFC 8628 §3.2: the polling interval when the server names none.
const DEFAULT_INTERVAL_SECS: u64 = 5;

/// RFC 8628 §3.5: on `slow_down`, the interval grows by 5 seconds, for this
/// and every later request.
const SLOW_DOWN_INCREMENT: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
struct DeviceAuthorizationResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

fn default_interval() -> u64 {
    DEFAULT_INTERVAL_SECS
}

/// What the person needs to approve a device login on another device. The
/// device code itself is deliberately absent: it is the credential being
/// polled with and is never shown.
#[derive(Debug, Clone)]
pub struct DeviceCodePrompt {
    pub user_code: String,
    pub verification_uri: String,
    /// The verification address with the user code already filled in, when
    /// the server offers one.
    pub verification_uri_complete: Option<String>,
    pub expires_in: Duration,
}

/// The polling state of one device login.
#[derive(Debug)]
pub(crate) struct DevicePoll {
    interval: Duration,
    waited: Duration,
    lifetime: Duration,
}

impl DevicePoll {
    pub(crate) fn new(interval: Duration, lifetime: Duration) -> Self {
        Self {
            interval,
            waited: Duration::ZERO,
            lifetime,
        }
    }

    /// How long to wait before the next poll, or the expiry if that wait
    /// would run past the device code's lifetime.
    ///
    /// The lifetime is counted in waits, not wall-clock time: the server's
    /// `expired_token` is authoritative, and this is only the backstop for a
    /// server that keeps answering `authorization_pending`.
    pub(crate) fn next_wait(&mut self) -> Result<Duration, LoginError> {
        if self.waited + self.interval > self.lifetime {
            return Err(LoginError::DeviceCodeExpired);
        }
        self.waited += self.interval;
        Ok(self.interval)
    }

    /// The outcome of one poll: `None` to keep polling, `Some` when the login
    /// is over either way.
    pub(crate) fn on_reply(
        &mut self,
        reply: Reply<TokenResponse>,
    ) -> Option<Result<TokenResponse, LoginError>> {
        let error = match reply {
            Ok(token) => return Some(Ok(token)),
            Err(error) => error,
        };
        match error.error.as_str() {
            "authorization_pending" => None,
            "slow_down" => {
                self.interval += SLOW_DOWN_INCREMENT;
                None
            }
            "expired_token" => Some(Err(LoginError::DeviceCodeExpired)),
            "access_denied" => Some(Err(LoginError::AccessDenied)),
            _ => Some(Err(error.into())),
        }
    }
}

/// Runs a device login: requests a device code, shows it, and polls until
/// the person approves, denies, or the code expires.
///
/// `sleep` is the wait between polls; production passes
/// `tokio::time::sleep`, tests pass one that records and returns at once.
pub(crate) async fn run<S, F>(
    http: &reqwest::Client,
    endpoints: &OidcEndpoints,
    client_id: &str,
    progress: &dyn LoginProgress,
    sleep: S,
) -> Result<TokenResponse, LoginError>
where
    S: Fn(Duration) -> F,
    F: Future<Output = ()>,
{
    // PKCE is not part of RFC 8628, but Keycloak applies a client's PKCE
    // requirement to the device grant too: with `S256` required, a device
    // request without a challenge is refused. A server that does not know
    // the parameters ignores them, so they are always sent.
    let pkce = Pkce::generate();
    let authorization: DeviceAuthorizationResponse = post_form(
        http,
        &endpoints.device_authorization,
        &[
            ("client_id", client_id),
            ("scope", "openid"),
            ("code_challenge", pkce.challenge()),
            ("code_challenge_method", CODE_CHALLENGE_METHOD),
        ],
    )
    .await?
    .map_err(LoginError::from)?;

    progress.show_device_code(&DeviceCodePrompt {
        user_code: authorization.user_code,
        verification_uri: authorization.verification_uri,
        verification_uri_complete: authorization.verification_uri_complete,
        expires_in: Duration::from_secs(authorization.expires_in),
    });

    let mut poll = DevicePoll::new(
        Duration::from_secs(authorization.interval),
        Duration::from_secs(authorization.expires_in),
    );
    loop {
        sleep(poll.next_wait()?).await;
        let reply = post_form(
            http,
            &endpoints.token,
            &[
                ("grant_type", DEVICE_CODE_GRANT_TYPE),
                ("device_code", authorization.device_code.as_str()),
                ("client_id", client_id),
                ("code_verifier", pkce.verifier()),
            ],
        )
        .await?;
        if let Some(outcome) = poll.on_reply(reply) {
            return outcome;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::OAuthErrorBody;

    fn token() -> TokenResponse {
        TokenResponse {
            access_token: "a".into(),
            expires_in: 300,
            refresh_token: "r".into(),
            refresh_expires_in: 1800,
            token_type: "Bearer".into(),
        }
    }

    fn error(code: &str) -> Reply<TokenResponse> {
        Err(OAuthErrorBody {
            error: code.into(),
            error_description: Some(format!("{code} description")),
        })
    }

    fn poll() -> DevicePoll {
        DevicePoll::new(Duration::from_secs(5), Duration::from_secs(600))
    }

    #[test]
    fn authorization_pending_keeps_polling_at_the_same_interval() {
        let mut poll = poll();
        assert_eq!(poll.next_wait().unwrap(), Duration::from_secs(5));
        assert!(poll.on_reply(error("authorization_pending")).is_none());
        assert_eq!(poll.next_wait().unwrap(), Duration::from_secs(5));
    }

    #[test]
    fn slow_down_adds_five_seconds_for_every_later_poll() {
        let mut poll = poll();
        assert!(poll.on_reply(error("slow_down")).is_none());
        assert_eq!(poll.next_wait().unwrap(), Duration::from_secs(10));
        assert!(poll.on_reply(error("authorization_pending")).is_none());
        assert_eq!(poll.next_wait().unwrap(), Duration::from_secs(10));
        assert!(poll.on_reply(error("slow_down")).is_none());
        assert_eq!(poll.next_wait().unwrap(), Duration::from_secs(15));
    }

    #[test]
    fn expired_token_ends_the_login_as_expired() {
        assert!(matches!(
            poll().on_reply(error("expired_token")),
            Some(Err(LoginError::DeviceCodeExpired))
        ));
    }

    #[test]
    fn access_denied_ends_the_login_as_denied() {
        assert!(matches!(
            poll().on_reply(error("access_denied")),
            Some(Err(LoginError::AccessDenied))
        ));
    }

    #[test]
    fn any_other_error_ends_the_login_naming_it() {
        let outcome = poll().on_reply(error("invalid_client"));
        assert!(matches!(
            outcome,
            Some(Err(LoginError::Refused { ref error, .. })) if error == "invalid_client"
        ));
    }

    #[test]
    fn a_token_ends_the_login_with_it() {
        assert!(matches!(
            poll().on_reply(Ok(token())),
            Some(Ok(t)) if t.access_token == "a"
        ));
    }

    /// A server that answers `authorization_pending` forever still ends: the
    /// waits stop at the code's lifetime.
    #[test]
    fn polling_stops_at_the_device_code_lifetime() {
        let mut poll = DevicePoll::new(Duration::from_secs(5), Duration::from_secs(12));
        assert!(poll.next_wait().is_ok());
        assert!(poll.next_wait().is_ok());
        assert!(matches!(
            poll.next_wait(),
            Err(LoginError::DeviceCodeExpired)
        ));
    }

    #[test]
    fn a_device_authorization_without_an_interval_polls_every_five_seconds() {
        let parsed: DeviceAuthorizationResponse = serde_json::from_str(
            r#"{"device_code":"d","user_code":"ABCD-EFGH",
                "verification_uri":"https://idp/device","expires_in":600}"#,
        )
        .unwrap();
        assert_eq!(parsed.interval, 5);
        assert!(parsed.verification_uri_complete.is_none());
    }
}
