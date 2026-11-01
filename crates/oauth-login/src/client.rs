use crate::browser::{self, BrowserLoginOptions};
use crate::device;
use crate::endpoints::OidcEndpoints;
use crate::error::LoginError;
use crate::progress::LoginProgress;
use crate::token::{post_form, TokenResponse};

/// A public (secret-less) OAuth client of one identity provider, as a native
/// app logs in with it.
#[derive(Debug, Clone)]
pub struct NativeLoginClient {
    http: reqwest::Client,
    endpoints: OidcEndpoints,
    client_id: String,
}

impl NativeLoginClient {
    pub fn new(endpoints: OidcEndpoints, client_id: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::default(),
            endpoints,
            client_id: client_id.into(),
        }
    }

    /// Logs in through the system browser: Authorization Code with PKCE
    /// (`S256`) and a loopback redirect to `127.0.0.1` on an OS-assigned port.
    /// The client's registered redirect URIs must accept that.
    pub async fn login_in_browser(
        &self,
        options: &BrowserLoginOptions,
        progress: &dyn LoginProgress,
    ) -> Result<TokenResponse, LoginError> {
        browser::run(
            &self.http,
            &self.endpoints,
            &self.client_id,
            options,
            progress,
        )
        .await
    }

    /// Logs in with the Device Authorization Grant: shows a code to approve
    /// on any other device, and polls until it is approved, denied or
    /// expires.
    pub async fn login_with_device_code(
        &self,
        progress: &dyn LoginProgress,
    ) -> Result<TokenResponse, LoginError> {
        device::run(
            &self.http,
            &self.endpoints,
            &self.client_id,
            progress,
            tokio::time::sleep,
        )
        .await
    }

    /// Exchanges a refresh token for new tokens. Works for a session from
    /// either login, and for one from a password grant before it.
    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenResponse, LoginError> {
        post_form(
            &self.http,
            &self.endpoints.token,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", self.client_id.as_str()),
            ],
        )
        .await?
        .map_err(LoginError::from)
    }
}
