//! The `origin` command-line interface for Origin Cloud.
//!
//! - `pvs` holds the persistent volume service's commands, generated from
//!   the API definitions and calling the Rust SDK;
//! - `handler` and `service_apis` hold the hand-written commands, which call
//!   the GraphQL API with the queries in `graphql/`.

pub use run_cli::run;

pub(crate) mod client_auth;
mod config;
mod confirm;
pub(crate) mod graphql;
mod handler;
mod identity;
mod output;
mod pvs;
mod root_command;
mod run_cli;
pub(crate) mod service_apis;

/// The `origin` command tree, for checking a command line against it
/// without running it. A tool that prints an `origin` command for someone
/// to run parses that command against this in its tests, so renaming a
/// flag here fails those tests instead of the printed command.
pub fn command() -> clap::Command {
    <root_command::Cli as clap::CommandFactory>::command()
}
