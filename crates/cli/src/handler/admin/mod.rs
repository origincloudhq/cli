//! `origin admin` subcommand group.
//!
//! `commands.rs` declares the clap surface (`AdministrationCommands`); `dispatch.rs`
//! matches each variant and performs the work.

mod commands;
mod dispatch;

pub(crate) use commands::AdministrationCommands;
pub use dispatch::run_admin_command;
