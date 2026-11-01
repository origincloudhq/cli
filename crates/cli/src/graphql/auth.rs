//! Where the GraphQL client gets the access token each call is sent with.

use std::future::Future;

#[derive(thiserror::Error, Debug)]
pub(crate) enum OAuthAuthenticationError {
    #[error("not authenticated: log in with `origin id login`")]
    Unauthenticated,
}

/// Supplies the access token of each call.
pub(crate) trait OAuthClientAuthenticator {
    fn get_token(
        &self,
    ) -> impl Future<Output = Result<impl TokenProvider + Send, OAuthAuthenticationError>> + Send;
}

/// An access token and its type, such as `Bearer`.
pub(crate) trait TokenProvider {
    fn token(&self) -> &str;
    fn ty(&self) -> &str;
}
