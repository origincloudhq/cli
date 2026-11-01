//! `origin id` subcommand group.
//!
//! `commands.rs` declares the clap surface (`IdentityCommands`); `dispatch.rs`
//! matches each variant and performs the work.

mod commands;
mod dispatch;
mod login;
#[cfg(test)]
mod tests;
mod token;

pub(crate) use commands::IdentityCommands;
pub use dispatch::run_identity_command;
