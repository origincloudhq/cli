//! `origin iam` subcommand group.
//!
//! `commands.rs` declares the clap surface (`IamCommands`); `dispatch.rs`
//! matches each variant and performs the work.

mod commands;
mod dispatch;

pub(crate) use commands::IamCommands;
pub use dispatch::run_iam_command;
