//! `origin network` subcommand group.
//!
//! `commands.rs` declares the clap surface (`NetworkCommands`); `dispatch.rs`
//! matches each variant and performs the work.

mod commands;
mod dispatch;
pub(crate) mod rule_args;
#[cfg(test)]
mod tests;

pub(crate) use commands::NetworkCommands;
pub use dispatch::run_network_command;
