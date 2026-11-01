use crate::config::ClientSession;
use crate::graphql::{OAuthAuthenticationError, OAuthClientAuthenticator, TokenProvider};
use origincloud_oauth_login::{
    BrowserLoginOptions, NativeLoginClient, OidcEndpoints, StderrPrompts, TokenResponse,
};

const IDENTITY_URL: &str = "https://auth.origincloud.eu";
const REALM_NAME: &str = "origincloud";
const OIDC_CLIENT_ID: &str = "origin-cli";

#[derive(Default)]
#[allow(dead_code)]
pub(crate) struct ConfigAuthenticator {}

impl OAuthClientAuthenticator for ConfigAuthenticator {
    async fn get_token(&self) -> Result<impl TokenProvider, OAuthAuthenticationError> {
        let credentials = ClientSession::read_and_validate_credentials()
            .await
            .ok_or(OAuthAuthenticationError::Unauthenticated)?;
        Ok(credentials)
    }
}

/// How `origin id login` authenticates. Neither way lets the CLI see the
/// person's password: they authenticate at the identity provider's own login
/// page, with whatever it offers.
pub(crate) enum LoginScheme {
    /// Authorization Code with PKCE through the system browser.
    Browser { login_hint: Option<String> },
    /// The Device Authorization Grant, for a host with no browser.
    Device,
}

fn login_client() -> anyhow::Result<NativeLoginClient> {
    let endpoints = OidcEndpoints::keycloak_realm(IDENTITY_URL, REALM_NAME)?;
    Ok(NativeLoginClient::new(endpoints, OIDC_CLIENT_ID))
}

/// Logs in and stores the session.
pub(crate) async fn login(scheme: LoginScheme) -> anyhow::Result<ClientSession> {
    let client = login_client()?;
    let token = match scheme {
        LoginScheme::Browser { login_hint } => {
            let options = BrowserLoginOptions {
                login_hint,
                ..BrowserLoginOptions::default()
            };
            client
                .login_in_browser(&options, &StderrPrompts::new("origin"))
                .await?
        }
        LoginScheme::Device => {
            client
                .login_with_device_code(&StderrPrompts::new("origin"))
                .await?
        }
    };
    store(token)
}

/// Renews a session from its refresh token and stores the result. A session
/// from any login refreshes the same way, including one written by the
/// password login this CLI used to have.
pub(crate) async fn refresh(refresh_token: &str) -> anyhow::Result<ClientSession> {
    let token = login_client()?.refresh(refresh_token).await?;
    store(token)
}

fn store(token: TokenResponse) -> anyhow::Result<ClientSession> {
    let session = ClientSession::from(token);
    session.safe_to_file()?;
    Ok(session)
}
