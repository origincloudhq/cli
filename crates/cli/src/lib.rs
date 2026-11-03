//! The `origin` command-line interface for Origin Cloud.
//!
//! - `sdk` holds the command groups generated from the API definitions,
//!   one per service (`pvs`, `location`, ...), which call the Rust SDK;
//! - `handler` and `service_apis` hold the hand-written commands, which call
//!   the GraphQL API with the queries in `graphql/`.

pub use run_cli::run;

pub(crate) mod client_auth;
mod config;
mod confirm;
mod descriptors;
pub(crate) mod graphql;
mod handler;
mod identity;
mod output;
mod pager;
mod progress;
mod root_command;
mod run_cli;
mod sdk;
pub(crate) mod service_apis;

/// The `origin` command tree, for checking a command line against it
/// without running it. A tool that prints an `origin` command for someone
/// to run parses that command against this in its tests, so renaming a
/// flag here fails those tests instead of the printed command.
pub fn command() -> clap::Command {
    <root_command::Cli as clap::CommandFactory>::command()
}
