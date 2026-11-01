use clap::builder::PossibleValuesParser;
use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub(crate) enum McpCommands {
    /// Serve Origin Cloud's API to an AI agent, as an MCP server on
    /// standard input and output.
    ///
    /// An MCP client, such as an AI coding assistant, starts it as a local
    /// server: `origin mcp serve`. It has a tool per method of the API, in a
    /// toolset per service, and calls them as this profile: with its login,
    /// or the credentials in ORIGINCLOUD_ACCESS_TOKEN, or
    /// ORIGINCLOUD_CLIENT_ID and ORIGINCLOUD_CLIENT_SECRET, its `region`,
    /// and each service's endpoint variable, such as
    /// ORIGINCLOUD_PVS_ENDPOINT. The tool `current_context` tells the agent
    /// the profile's `default-project` and `default-zone`.
    ///
    /// The tools that delete something, or change it for good, are marked
    /// destructive for the client to confirm: they don't ask for `--yes`.
    /// `--read-only` leaves out every tool that changes anything.
    Serve(ServeArgs),
}

#[derive(Args, Debug)]
pub(crate) struct ServeArgs {
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
