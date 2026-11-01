//! `origin compute` subcommand group.
//!
//! `commands.rs` declares the clap surface (`ComputeCommands`); `dispatch.rs`
//! matches each variant and performs the work.

mod addresses;
mod commands;
mod dispatch;
#[cfg(test)]
mod tests;

pub(crate) use commands::ComputeCommands;
pub use dispatch::run_compute_command;
