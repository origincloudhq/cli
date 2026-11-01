//! The SDK's credentials over the CLI's own login session.
//!
//! The SDK can read the session file itself, but renews it only in memory.
//! This adapter renews it the way every other command does, through
//! `origin id login`'s client, under the session's lock, and stores the
//! renewed session, so that the generated commands and the GraphQL commands
//! share one login.

use std::fmt;

use chrono::{DateTime, Utc};
use origincloud_core::credentials::{Credentials, TokenFuture};
use origincloud_core::Error;
use tokio::sync::Mutex;

use crate::config::{ClientSession, SessionError, EXPIRY_MARGIN};
use crate::graphql::TokenProvider;

/// What a command says when it has no credentials at all.
pub(crate) const NOT_LOGGED_IN: &str = "not logged in: log in with `origin id login`, or set \
    ORIGINCLOUD_ACCESS_TOKEN, or ORIGINCLOUD_CLIENT_ID and ORIGINCLOUD_CLIENT_SECRET";

/// The access token of the CLI's login session, renewed from its refresh
/// token, and stored, once it is about to expire.
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
            let session = ClientSession::valid(EXPIRY_MARGIN)
                .await
                .map_err(|e| match e {
                    SessionError::None => Error::Credentials(NOT_LOGGED_IN.to_owned()),
                    other => Error::Credentials(other.to_string()),
                })?;
            let token = session.token().to_owned();
            *cached = Some((token.clone(), session.expires()));
            Ok(token)
        })
    }
}
