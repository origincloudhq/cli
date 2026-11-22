//! The SDK's credentials over the CLI's own login session, and, without
//! one, the instance's metadata service. A service account key's
//! credentials file, which `ORIGINCLOUD_CREDENTIALS_FILE` or the profile's
//! `credentials-file` names, comes first, and then, in a CI job whose OIDC
//! token the environment names, that token exchanged at the STS: the chain
//! every SDK follows.
//!
//! The SDK can read the session file itself, but renews it only in memory.
//! This adapter renews it the way every other command does, through
//! `origin id login`'s client, under the session's lock, and stores the
//! renewed session, so that the generated commands and the GraphQL commands
//! share one login. On an instance that runs as a service account, with no
//! login session, the commands call the API as that service account.

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use chrono::{DateTime, Utc};
use origincloud_core::credentials::{Credentials, TokenFuture};
use origincloud_core::{Error, Federated, InstanceMetadata, ServiceAccount};
use tokio::sync::Mutex;

use crate::config::{ClientSession, SessionError, EXPIRY_MARGIN};
use crate::graphql::TokenProvider;

/// What a command says when it has no credentials at all.
pub(crate) const NOT_LOGGED_IN: &str = "not logged in: log in with `origin id login`, or set \
    ORIGINCLOUD_ACCESS_TOKEN, or name a service account key's credentials file with \
    ORIGINCLOUD_CREDENTIALS_FILE or `origin config profile-set credentials-file FILE`, or run in a \
    CI job whose OIDC token a federated credential trusts (GitHub Actions with `permissions: \
    id-token: write`, ORIGINCLOUD_OIDC_TOKEN_FILE or ORIGINCLOUD_OIDC_TOKEN), or run on an \
    instance that runs as a \
    service account";

const ENV_CREDENTIALS_FILE: &str = "ORIGINCLOUD_CREDENTIALS_FILE";
const ENV_METADATA_DISABLED: &str = "ORIGINCLOUD_METADATA_DISABLED";
const ENV_METADATA_ENDPOINT: &str = "ORIGINCLOUD_METADATA_ENDPOINT";
const ENV_OIDC_DISABLED: &str = "ORIGINCLOUD_OIDC_DISABLED";
const ENV_OIDC_TOKEN_FILE: &str = "ORIGINCLOUD_OIDC_TOKEN_FILE";
const ENV_OIDC_TOKEN: &str = "ORIGINCLOUD_OIDC_TOKEN";
const ENV_OIDC_AUDIENCE: &str = "ORIGINCLOUD_OIDC_AUDIENCE";
const ENV_STS_ENDPOINT: &str = "ORIGINCLOUD_STS_ENDPOINT";
const ENV_GITHUB_REQUEST_URL: &str = "ACTIONS_ID_TOKEN_REQUEST_URL";
const ENV_GITHUB_REQUEST_TOKEN: &str = "ACTIONS_ID_TOKEN_REQUEST_TOKEN";

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// The credentials file the profile names, `ORIGINCLOUD_CREDENTIALS_FILE`
/// before its `credentials-file`, as [`configure`] was given it.
static CREDENTIALS_FILE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Sets the credentials file of the process's commands to the one
/// `profile` names, its variable before its file's value. `origin` calls it
/// once, with the profile it loaded, before running a command; without
/// it, the variable alone names one.
pub(crate) fn configure(profile: &crate::config::ClientProfile) {
    let _ = CREDENTIALS_FILE.set(profile.credentials_file.clone());
}

/// The service account whose key's credentials file the profile names, read
/// once per process, so that its commands share one token, which the SDK
/// renews before it expires; `None` when no file is named. A file that
/// can't be read, or isn't a credentials file, is the error: it was named
/// to be used, and the commands don't fall back to another identity.
pub(crate) fn service_account() -> Result<Option<Arc<ServiceAccount>>, Error> {
    static ACCOUNT: OnceLock<Option<Result<Arc<ServiceAccount>, String>>> = OnceLock::new();
    let account = ACCOUNT.get_or_init(|| {
        let path = match CREDENTIALS_FILE.get() {
            Some(configured) => configured.clone(),
            None => var(ENV_CREDENTIALS_FILE).map(PathBuf::from),
        }?;
        Some(ServiceAccount::from_file(&path).map(Arc::new).map_err(|e| {
            format!(
                "the credentials file {}, from ORIGINCLOUD_CREDENTIALS_FILE or the \
                         profile's credentials-file: {e}",
                path.display()
            )
        }))
    });
    match account {
        None => Ok(None),
        Some(Ok(account)) => Ok(Some(Arc::clone(account))),
        Some(Err(message)) => Err(Error::Credentials(message.clone())),
    }
}

/// The CI job's OIDC token, exchanged at the STS, when the environment
/// names one as the SDKs find it: the file `ORIGINCLOUD_OIDC_TOKEN_FILE`
/// names, or else the token `ORIGINCLOUD_OIDC_TOKEN` holds (as GitLab CI's
/// `id_tokens` sets it), or else GitHub Actions' token in a job granted
/// `permissions: id-token: write`; `None` without one, or with
/// `ORIGINCLOUD_OIDC_DISABLED=true`. One per process, so its commands
/// share one exchange.
pub(crate) fn ci_job() -> Option<&'static Federated> {
    static CI_JOB: OnceLock<Option<Federated>> = OnceLock::new();
    CI_JOB
        .get_or_init(|| {
            if var(ENV_OIDC_DISABLED).is_some_and(|v| v.eq_ignore_ascii_case("true")) {
                return None;
            }
            let mut federated = match (
                var(ENV_OIDC_TOKEN_FILE),
                var(ENV_OIDC_TOKEN),
                var(ENV_GITHUB_REQUEST_URL),
                var(ENV_GITHUB_REQUEST_TOKEN),
            ) {
                (Some(path), _, _, _) => Federated::from_file(path),
                (None, Some(token), _, _) => Federated::from_token(token),
                (None, None, Some(url), Some(token)) => Federated::github_actions(url, token),
                _ => return None,
            };
            if let Some(endpoint) = var(ENV_STS_ENDPOINT) {
                federated = federated.with_endpoint(endpoint);
            }
            if let Some(audience) = var(ENV_OIDC_AUDIENCE) {
                federated = federated.with_audience(audience);
            }
            Some(federated)
        })
        .as_ref()
}

/// The instance's metadata service, which the commands ask for the
/// credential of the service account the instance runs as when there is no
/// login session; `None` when `ORIGINCLOUD_METADATA_DISABLED=true` leaves
/// it out. `ORIGINCLOUD_METADATA_ENDPOINT` names another address than the
/// metadata service's own, as it does for the SDKs.
fn instance_metadata() -> Option<&'static InstanceMetadata> {
    static METADATA: OnceLock<Option<InstanceMetadata>> = OnceLock::new();
    METADATA
        .get_or_init(|| {
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
    /// A service account's: its key's, the CI job's, or the instance's.
    Workload(String),
}

impl TokenProvider for AccessToken {
    fn token(&self) -> &str {
        match self {
            Self::Session(session) => session.token(),
            Self::Workload(token) => token,
        }
    }

    fn ty(&self) -> &str {
        match self {
            Self::Session(session) => session.ty(),
            Self::Workload(_) => "Bearer",
        }
    }
}

/// The service account's token, exchanged at the STS for an assertion
/// signed with its key, when the profile names a credentials file; else
/// the CI job's token, exchanged at the STS, when the environment names
/// one; otherwise the login session's access token, renewed when it is
/// about to expire, or, without a login session, the instance's
/// credential. A refused exchange is the error: whoever named the file or
/// set up federation wants to hear why it failed.
pub(crate) async fn access_token() -> Result<AccessToken, Error> {
    if let Some(account) = service_account()? {
        return account.token().await.map(AccessToken::Workload);
    }
    if let Some(ci_job) = ci_job() {
        return ci_job.token().await.map(AccessToken::Workload);
    }
    match ClientSession::valid(EXPIRY_MARGIN).await {
        Ok(session) => Ok(AccessToken::Session(session)),
        Err(SessionError::None) => instance_token().await.map(AccessToken::Workload),
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
/// the instance's credential, which its metadata service renews. With a
/// credentials file, or in a CI job, the exchanged token, which
/// [`service_account`]'s or [`ci_job`]'s credentials renew.
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
                AccessToken::Workload(token) => Ok(token),
            }
        })
    }
}
