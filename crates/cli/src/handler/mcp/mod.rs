//! `origin mcp` subcommand group: Origin Cloud's MCP tools (`origincloud-mcp`,
//! from origincloudhq/mcp) served to an agent on this machine, and that
//! server registered with the coding agents.
//!
//! `commands.rs` declares the clap surface (`McpCommands`); `dispatch.rs`
//! serves; `install.rs` adds the server to an agent and removes it, with
//! the table of agents in `agents.rs` and the edits of their files in
//! `config_file.rs`.

mod agents;
mod commands;
mod config_file;
mod dispatch;
mod install;

pub(crate) use commands::McpCommands;
pub use dispatch::run_mcp_command;
pub(crate) use install::run_install_command;
