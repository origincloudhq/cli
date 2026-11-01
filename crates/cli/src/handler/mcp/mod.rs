//! `origin mcp` subcommand group: Origin Cloud's MCP tools (`crates/mcp`)
//! served to an agent on this machine.
//!
//! `commands.rs` declares the clap surface (`McpCommands`); `dispatch.rs`
//! matches each variant and performs the work.

mod commands;
mod dispatch;

pub(crate) use commands::McpCommands;
pub use dispatch::run_mcp_command;
