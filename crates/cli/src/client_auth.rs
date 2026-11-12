use crate::config::{Assumed, ClientSession};
use crate::graphql::{OAuthAuthenticationError, OAuthClientAuthenticator, TokenProvider};
use crate::sdk::credentials::access_token;
use crate::sdk::read_value_file;
use chrono::{DateTime, Utc};
use origincloud_core::credentials::{Credentials, TokenFuture};
use origincloud_oauth_login::{
    BrowserLoginOptions, NativeLoginClient, OidcEndpoints, StderrPrompts, TokenResponse,
};
use origincloud_sts::v1beta1::{AssumeServiceAccountRequest, SecurityTokenClient};

const IDENTITY_URL: &str = "https://auth.origincloud.eu";
const REALM_NAME: &str = "origincloud";
const OIDC_CLIENT_ID: &str = "origin-cli";

#[derive(Default)]
#[allow(dead_code)]
pub(crate) struct ConfigAuthenticator {}

impl OAuthClientAuthenticator for ConfigAuthenticator {
    async fn get_token(&self) -> Result<impl TokenProvider, OAuthAuthenticationError> {
        access_token()
            .await
            .map_err(|_| OAuthAuthenticationError::Unauthenticated)
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

/// Where the security token service is, when not at its own host: the
/// variable the `sts` commands and the CI job's exchange take too.
const ENV_STS_ENDPOINT: &str = "ORIGINCLOUD_STS_ENDPOINT";

/// No credentials: the security token service authenticates no caller,
/// and the SDK sends its calls without asking for any.
#[derive(Debug)]
struct NoCredentials;

impl Credentials for NoCredentials {
    fn token(&self) -> TokenFuture<'_> {
        Box::pin(async { Err(origincloud_core::Error::NoCredentials) })
    }
}

/// Assumes a service account with the identity provider's token in
/// `token_file`, or on standard input for `-`, and stores the session:
/// `service_account`, or the one whose federated credential trusts the
/// token. A file's session is assumed again from it once it expires.
pub(crate) async fn login_with_token(
    token_file: &str,
    service_account: Option<String>,
) -> anyhow::Result<ClientSession> {
    let token_file = match token_file {
        "-" => None,
        path => Some(std::path::absolute(path)?),
    };
    let token = read_token(token_file.as_deref())?;
    let session = exchange(token, service_account.unwrap_or_default(), token_file).await?;
    session.safe_to_file()?;
    Ok(session)
}

/// Assumes the service account of a session that expired again, with the
/// token its file holds now, and stores the result.
pub(crate) async fn assume(assumed: Assumed) -> anyhow::Result<ClientSession> {
    let Some(path) = assumed.token_file else {
        anyhow::bail!("the token came from standard input: log in again with `origin id login`");
    };
    let token = read_token(Some(&path))?;
    let session = exchange(token, assumed.service_account, Some(path)).await?;
    session.safe_to_file()?;
    Ok(session)
}

/// The token in the file at `path`, or on standard input without one.
fn read_token(path: Option<&std::path::Path>) -> anyhow::Result<String> {
    match path {
        None => read_value_file("-"),
        Some(path) => read_value_file(
            path.to_str()
                .ok_or_else(|| anyhow::anyhow!("the token file's path isn't UTF-8"))?,
        ),
    }
}

async fn exchange(
    token: String,
    service_account: String,
    token_file: Option<std::path::PathBuf>,
) -> anyhow::Result<ClientSession> {
    let mut builder = origincloud_core::Client::builder()
        .user_agent(format!("origin-cli/{}", env!("CARGO_PKG_VERSION")))
        .credentials(NoCredentials);
    if let Some(endpoint) = std::env::var(ENV_STS_ENDPOINT)
        .ok()
        .filter(|v| !v.is_empty())
    {
        builder = builder.endpoint(endpoint);
    }
    let client = builder.build()?;
    let response = SecurityTokenClient::new(&client)
        .assume_service_account(AssumeServiceAccountRequest {
            oidc_token: token,
            service_account,
            lifetime: None,
        })
        .await?;
    let expires = response
        .expire_time
        .and_then(|t| DateTime::<Utc>::from_timestamp(t.seconds, u32::try_from(t.nanos).ok()?))
        .ok_or_else(|| anyhow::anyhow!("the security token service answered with no expiry"))?;
    Ok(ClientSession::assumed(
        response.access_token,
        expires,
        Assumed {
            token_file,
            service_account: response.service_account,
        },
    ))
}
