//! `origin config` subcommand group.
//!
//! `commands.rs` declares the clap surface (`ConfigCommands`); `dispatch.rs`
//! matches each variant and performs the work.

mod commands;
mod dispatch;
mod profile_set;

pub(crate) use commands::ConfigCommands;
pub use dispatch::run_config_command;
