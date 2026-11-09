//! The SDK's credentials over the CLI's own login session, and, without
//! one, the instance's metadata service.
//!
//! The SDK can read the session file itself, but renews it only in memory.
//! This adapter renews it the way every other command does, through
//! `origin id login`'s client, under the session's lock, and stores the
//! renewed session, so that the generated commands and the GraphQL commands
//! share one login. On an instance that runs as a service account, with no
//! login session, the commands call the API as that service account.

use std::fmt;
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use origincloud_core::credentials::{Credentials, TokenFuture};
use origincloud_core::{Error, InstanceMetadata};
use tokio::sync::Mutex;

use crate::config::{ClientSession, SessionError, EXPIRY_MARGIN};
use crate::graphql::TokenProvider;

/// What a command says when it has no credentials at all.
pub(crate) const NOT_LOGGED_IN: &str = "not logged in: log in with `origin id login`, or set \
    ORIGINCLOUD_ACCESS_TOKEN, or ORIGINCLOUD_CLIENT_ID and ORIGINCLOUD_CLIENT_SECRET, or run on \
    an instance that runs as a service account";

const ENV_METADATA_DISABLED: &str = "ORIGINCLOUD_METADATA_DISABLED";
const ENV_METADATA_ENDPOINT: &str = "ORIGINCLOUD_METADATA_ENDPOINT";

/// The instance's metadata service, which the commands ask for the
/// credential of the service account the instance runs as when there is no
/// login session; `None` when `ORIGINCLOUD_METADATA_DISABLED=true` leaves
/// it out. `ORIGINCLOUD_METADATA_ENDPOINT` names another address than the
/// metadata service's own, as it does for the SDKs.
fn instance_metadata() -> Option<&'static InstanceMetadata> {
    static METADATA: OnceLock<Option<InstanceMetadata>> = OnceLock::new();
    METADATA
        .get_or_init(|| {
            let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
            if var(ENV_METADATA_DISABLED).is_some_and(|v| v.eq_ignore_ascii_case("true")) {
                return None;
            }
            let metadata = InstanceMetadata::new();
            Some(match var(ENV_METADATA_ENDPOINT) {
                Some(endpoint) => metadata.with_endpoint(endpoint),
                None => metadata,
            })
        })
        .as_ref()
}

/// An access token of the CLI's credentials, as `origin id token` prints
/// it and the GraphQL commands send it.
pub(crate) enum AccessToken {
    Session(ClientSession),
    Instance(String),
}

impl TokenProvider for AccessToken {
    fn token(&self) -> &str {
        match self {
            Self::Session(session) => session.token(),
            Self::Instance(token) => token,
        }
    }

    fn ty(&self) -> &str {
        match self {
            Self::Session(session) => session.ty(),
            Self::Instance(_) => "Bearer",
        }
    }
}

/// The login session's access token, renewed when it is about to expire,
/// or, without a login session, the instance's credential.
pub(crate) async fn access_token() -> Result<AccessToken, Error> {
    match ClientSession::valid(EXPIRY_MARGIN).await {
        Ok(session) => Ok(AccessToken::Session(session)),
        Err(SessionError::None) => instance_token().await.map(AccessToken::Instance),
        Err(other) => Err(Error::Credentials(other.to_string())),
    }
}

/// The instance's credential: [`NOT_LOGGED_IN`] off an instance, or with
/// the metadata service left out.
async fn instance_token() -> Result<String, Error> {
    let metadata = instance_metadata().ok_or_else(not_logged_in)?;
    metadata.token().await.map_err(|e| match e {
        Error::NoCredentials => not_logged_in(),
        other => other,
    })
}

fn not_logged_in() -> Error {
    Error::Credentials(NOT_LOGGED_IN.to_owned())
}

/// The access token of the CLI's login session, renewed from its refresh
/// token, and stored, once it is about to expire. Without a login session,
/// the instance's credential, which its metadata service renews.
#[derive(Default)]
pub(crate) struct CliSessionCredentials {
    cached: Mutex<Option<(String, DateTime<Utc>)>>,
}

impl fmt::Debug for CliSessionCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CliSessionCredentials(..)")
    }
}

impl Credentials for CliSessionCredentials {
    fn token(&self) -> TokenFuture<'_> {
        Box::pin(async move {
            let mut cached = self.cached.lock().await;
            if let Some((token, expires)) = cached.as_ref() {
                if Utc::now() + EXPIRY_MARGIN < *expires {
                    return Ok(token.clone());
                }
            }
            match access_token().await? {
                AccessToken::Session(session) => {
                    let token = session.token().to_owned();
                    *cached = Some((token.clone(), session.expires()));
                    Ok(token)
                }
                AccessToken::Instance(token) => Ok(token),
            }
        })
    }
}
