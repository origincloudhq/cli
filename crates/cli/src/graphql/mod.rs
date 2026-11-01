//! The GraphQL client the hand-written commands (`admin`, `iam`, `compute`,
//! `network`, `storage`) send their queries with.

mod auth;
mod client;

pub(crate) use auth::{OAuthAuthenticationError, OAuthClientAuthenticator, TokenProvider};
pub(crate) use client::*;
