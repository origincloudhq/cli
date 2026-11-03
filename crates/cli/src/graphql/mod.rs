//! The GraphQL client `origin id login` reads the signed-in user with, and
//! the access tokens every command is sent with.

mod auth;
mod client;

pub(crate) use auth::{OAuthAuthenticationError, OAuthClientAuthenticator, TokenProvider};
pub(crate) use client::*;
