//! An access token and its type, as `origin id token` prints them.

/// An access token and its type, such as `Bearer`.
pub(crate) trait TokenProvider {
    fn token(&self) -> &str;
    fn ty(&self) -> &str;
}
