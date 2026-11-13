use clap::builder::PossibleValuesParser;
use clap::{Args, Subcommand};

use super::install::DEFAULT_NAME;
use crate::handler::agents::{self, Scope};

#[derive(Subcommand, Debug)]
pub(crate) enum McpCommands {
    /// Serve Origin Cloud's API to an AI agent, as an MCP server on
    /// standard input and output.
    ///
    /// An MCP client, such as an AI coding assistant, starts it as a local
    /// server: `origin mcp serve`. It has a tool per method of the API, in a
    /// toolset per service, and calls them as this profile: with its login,
    /// or the credentials in ORIGINCLOUD_ACCESS_TOKEN, or
    /// ORIGINCLOUD_CLIENT_ID and ORIGINCLOUD_CLIENT_SECRET, in the region
    /// the commands use (ORIGINCLOUD_REGION, else the profile's `region`,
    /// else the region of the default zone), and with each service's
    /// endpoint variable, such as ORIGINCLOUD_PVS_ENDPOINT. The tool
    /// `current_context` tells the agent the profile's `default-project`
    /// and `default-zone`, and that region.
    ///
    /// The tools that delete something, or change it for good, are marked
    /// destructive for the client to confirm: they don't ask for `--yes`.
    /// `--read-only` leaves out every tool that changes anything.
    Serve(ServeArgs),

    /// Add `origin mcp serve` to a coding agent's MCP servers.
    ///
    /// Adds an entry, named `origincloud` unless `--name` says otherwise,
    /// that starts `origin mcp serve` with `--toolsets` and `--read-only`,
    /// and with `--config` when it was given (`origin --config FILE mcp
    /// install`). Where the agent has a command for this, such as `claude
    /// mcp add`, and it is on PATH, it runs that; otherwise it edits the
    /// agent's file, keeping everything else in it, comments included where
    /// the format has them, after copying it to `FILE.origin.bak`. Running
    /// it again updates the entry; a file that doesn't parse, or an entry of
    /// that name that doesn't start `origin mcp serve`, is left as it is.
    ///
    /// The entry holds no credentials: `origin mcp serve` uses the login
    /// and the profile, as the other commands do, so a project's file can be
    /// committed. On a terminal, without `--agent`, it offers the agents it
    /// finds on this machine.
    #[command(after_long_help = agents::help_table())]
    Install(InstallArgs),

    /// Remove `origin mcp serve` from a coding agent's MCP servers.
    ///
    /// Removes the entry `origin mcp install` added, by its name, and
    /// nothing else, with the agent's command or by editing its file as
    /// `install` does. An entry of that name that doesn't start `origin mcp
    /// serve` is left as it is.
    #[command(after_long_help = agents::help_table())]
    Uninstall(UninstallArgs),
}

/// The flags of `origin mcp serve` that narrow what it serves, which
/// `origin mcp install` passes on.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct ServeFlags {
    /// The toolsets to expose, by service; every one when not given.
    #[arg(
        long = "toolsets",
        value_name = "TOOLSET",
        value_delimiter = ',',
        value_parser = PossibleValuesParser::new(origincloud_mcp::toolsets())
    )]
    pub toolsets: Vec<String>,
    /// Expose only the tools that change nothing: reads, and waiting for
    /// operations.
    #[arg(long = "read-only")]
    pub read_only: bool,
}

#[derive(Args, Debug)]
pub(crate) struct ServeArgs {
    #[command(flatten)]
    pub flags: ServeFlags,
}

/// Which agents, where, and under which name.
#[derive(Args, Debug, Clone)]
pub(crate) struct Target {
    /// The coding agents, comma-separated; on a terminal without it, those
    /// found on this machine are offered.
    #[arg(
        long = "agent",
        value_name = "AGENT",
        value_delimiter = ',',
        value_parser = PossibleValuesParser::new(agents::possible_values())
    )]
    pub agents: Vec<String>,
    /// `user`, the agent's own configuration, for every project; or
    /// `project`, the project in the current directory, in a file that is
    /// usually committed with it.
    #[arg(long, value_enum, default_value = "user")]
    pub scope: Scope,
    /// The name of the entry.
    #[arg(long, default_value = DEFAULT_NAME)]
    pub name: String,
}

#[derive(Args, Debug)]
pub(crate) struct InstallArgs {
    #[command(flatten)]
    pub target: Target,
    #[command(flatten)]
    pub serve: ServeFlags,
    /// Name the `origin` binary by its absolute path rather than as
    /// `origin`, for an agent that doesn't start it with the PATH of your
    /// shell. The agents started from a desktop launcher, such as Claude
    /// Desktop, always get it.
    #[arg(long)]
    pub absolute_path: bool,
    /// Print the entry, as the agent's file would hold it, and change
    /// nothing. Without `--agent`, in the `mcpServers` form most agents
    /// read.
    #[arg(long)]
    pub print: bool,
}

#[derive(Args, Debug)]
pub(crate) struct UninstallArgs {
    #[command(flatten)]
    pub target: Target,
}
