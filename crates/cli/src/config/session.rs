use super::file_io::{create_private_dir, create_private_file, read_file_opt, safe_file};
use super::paths::{self, ENV_DATA_DIR_USER};
use crate::client_auth::refresh;
use crate::config::env::{env_or, expect_env};
use crate::graphql::TokenProvider;
use chrono::{DateTime, Duration, Utc};
use origincloud_oauth_login::TokenResponse;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};

const FILE_NAME_SESSION: &str = "session.json";
/// Held while a session is renewed, so that two processes renewing at once
/// don't each spend the refresh token and overwrite each other's session.
const FILE_NAME_LOCK: &str = "session.lock";

/// How long before its expiry a session is renewed, so that its token
/// doesn't expire on the way, or during a long wait for an operation.
pub(crate) const EXPIRY_MARGIN: Duration = Duration::seconds(30);

fn data_base_dir() -> String {
    env_or(
        ENV_DATA_DIR_USER,
        format!("{}/.local/share", expect_env::<String>("HOME")),
    )
}

fn session_path() -> PathBuf {
    paths::path(data_base_dir().as_str(), FILE_NAME_SESSION)
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct ClientSession {
    version: u8,
    access_token: String,
    expires: DateTime<Utc>,
    refresh_token: String,
    refresh_expires: DateTime<Utc>,
    token_type: String,
}

/// Hand-written so that neither token can be printed.
impl fmt::Debug for ClientSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientSession")
            .field("version", &self.version)
            .field("access_token", &"*****")
            .field("expires", &self.expires)
            .field("refresh_token", &"*****")
            .field("refresh_expires", &self.refresh_expires)
            .field("token_type", &self.token_type)
            .finish()
    }
}

/// Why there is no usable session.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SessionError {
    #[error("not logged in: log in with `origin id login`")]
    None,
    #[error("the login session has expired: log in again with `origin id login`")]
    Expired,
    #[error("renewing the login session: {0}")]
    Renewal(String),
    #[error("locking the login session: {0}")]
    Lock(#[from] std::io::Error),
}

impl ClientSession {
    pub fn safe_to_file(&self) -> Result<(), std::io::Error> {
        safe_file(&session_path(), self)
    }

    /// When the access token expires.
    pub fn expires(&self) -> DateTime<Utc> {
        self.expires
    }

    /// Whether the access token is good for longer than `margin`.
    fn fresh(&self, margin: Duration) -> bool {
        !self.access_token.is_empty() && Utc::now() + margin < self.expires
    }

    /// The stored session, renewed when its token expires within `margin`.
    pub async fn valid(margin: Duration) -> Result<ClientSession, SessionError> {
        Self::valid_at(&session_path(), margin, |refresh_token| async move {
            refresh(&refresh_token)
                .await
                .map_err(|e| SessionError::Renewal(e.to_string()))
        })
        .await
    }

    /// [`Self::valid`] for the session at `path`, renewing it with `renew`,
    /// which stores the renewed session.
    async fn valid_at<F, Fut>(
        path: &Path,
        margin: Duration,
        renew: F,
    ) -> Result<ClientSession, SessionError>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<ClientSession, SessionError>>,
    {
        let session: ClientSession = read_file_opt(path).ok_or(SessionError::None)?;
        if session.fresh(margin) {
            return Ok(session);
        }
        let _lock = lock(path.with_file_name(FILE_NAME_LOCK)).await?;
        // Read again under the lock: another process may have renewed the
        // session while this one waited, and its refresh token may be the
        // only one still valid.
        let session: ClientSession = read_file_opt(path).ok_or(SessionError::None)?;
        if session.fresh(margin) {
            return Ok(session);
        }
        if session.refresh_expires <= Utc::now() {
            return Err(SessionError::Expired);
        }
        renew(session.refresh_token).await
    }

    pub async fn read_and_validate_credentials() -> Option<ClientSession> {
        Self::valid(EXPIRY_MARGIN).await.ok()
    }
}

/// An exclusive lock on the file at `path`, held until the file is dropped.
async fn lock(path: PathBuf) -> Result<File, SessionError> {
    tokio::task::spawn_blocking(move || {
        if let Some(dir) = path.parent() {
            create_private_dir(dir)?;
        }
        let file = create_private_file(&path, false)?;
        file.lock()?;
        Ok::<_, std::io::Error>(file)
    })
    .await
    .map_err(|e| SessionError::Lock(std::io::Error::other(e)))?
    .map_err(SessionError::Lock)
}

impl TokenProvider for ClientSession {
    fn token(&self) -> &str {
        self.access_token.as_str()
    }
    fn ty(&self) -> &str {
        self.token_type.as_str()
    }
}

impl From<TokenResponse> for ClientSession {
    fn from(value: TokenResponse) -> Self {
        let now = Utc::now();
        ClientSession {
            version: 1,
            access_token: value.access_token,
            expires: now + Duration::seconds(value.expires_in),
            refresh_token: value.refresh_token,
            refresh_expires: now + Duration::seconds(value.refresh_expires_in),
            token_type: value.token_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(token: &str, expires: DateTime<Utc>) -> ClientSession {
        ClientSession {
            version: 1,
            access_token: token.to_owned(),
            expires,
            refresh_token: format!("{token}-refresh"),
            refresh_expires: Utc::now() + Duration::hours(1),
            token_type: "Bearer".to_owned(),
        }
    }

    #[test]
    fn debug_hides_the_tokens() {
        let printed = format!("{:?}", session("secret-token", Utc::now()));
        assert!(!printed.contains("secret-token"), "{printed}");
    }

    #[tokio::test]
    async fn a_fresh_session_is_used_as_stored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME_SESSION);
        safe_file(&path, &session("stored", Utc::now() + Duration::hours(1))).unwrap();

        let found = ClientSession::valid_at(&path, EXPIRY_MARGIN, |_| async {
            panic!("a fresh session isn't renewed")
        })
        .await
        .unwrap();
        assert_eq!(found.token(), "stored");
    }

    /// Another process renews the session while this one waits for the lock:
    /// this one takes the renewed session instead of renewing again with a
    /// refresh token that may be spent.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_session_renewed_meanwhile_is_taken_instead_of_renewing_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME_SESSION);
        safe_file(
            &path,
            &session("expired", Utc::now() - Duration::minutes(1)),
        )
        .unwrap();

        let held = lock(path.with_file_name(FILE_NAME_LOCK)).await.unwrap();
        let waiting = {
            let path = path.clone();
            tokio::spawn(async move {
                ClientSession::valid_at(&path, EXPIRY_MARGIN, |_| async {
                    Err(SessionError::Renewal("renewed twice".to_owned()))
                })
                .await
            })
        };
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        safe_file(&path, &session("renewed", Utc::now() + Duration::hours(1))).unwrap();
        drop(held);

        let found = waiting.await.unwrap().unwrap();
        assert_eq!(found.token(), "renewed");
    }

    #[tokio::test]
    async fn an_expired_session_past_its_refresh_expiry_asks_to_log_in() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME_SESSION);
        let mut old = session("old", Utc::now() - Duration::hours(2));
        old.refresh_expires = Utc::now() - Duration::hours(1);
        safe_file(&path, &old).unwrap();

        let err = ClientSession::valid_at(&path, EXPIRY_MARGIN, |_| async {
            panic!("an expired refresh token isn't used")
        })
        .await
        .unwrap_err();
        assert!(err.to_string().contains("origin id login"), "{err}");
        assert!(matches!(
            ClientSession::valid_at(&dir.path().join("none.json"), EXPIRY_MARGIN, |_| async {
                panic!()
            })
            .await,
            Err(SessionError::None)
        ));
    }
}
