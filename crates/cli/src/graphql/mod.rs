//! The GraphQL client the hand-written commands (`iam`, and `id`'s current
//! user) send their queries with.

mod auth;
mod client;

pub(crate) use auth::{OAuthAuthenticationError, OAuthClientAuthenticator, TokenProvider};
pub(crate) use client::*;
