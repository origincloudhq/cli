use std::path::PathBuf;

use crate::config::OutputFormat;
use crate::handler;
use crate::handler::iam::IamCommands;
use crate::handler::id::IdentityCommands;
use crate::handler::mcp::McpCommands;
use crate::output::ColorWhen;
use crate::sdk::ServiceCommands;
use clap::{Parser, Subcommand};

#[derive(Parser)]
// `name` is set explicitly because clap derives it from CARGO_PKG_NAME, and
// the crate's name isn't the binary's, `origin`: `--version` would print the
// crate's.
#[command(name = "origin", version, about, long_about = None)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Path of the CLI configuration file to use instead of the default one.
    #[arg(short, long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,

    /// GraphQL API endpoint the hand-written commands send their requests
    /// to.
    #[arg(short, long, value_name = "URL", default_value_t = get_default_api_url())]
    pub(crate) api: String,

    /// Output format. Overrides ORIGINCLOUD_OUTPUT_FORMAT and the profile's
    /// `output-format`; without any of them, `table` on a terminal and
    /// `json` when the output goes to a pipe or a file.
    #[arg(
        short,
        long,
        value_name = "FORMAT",
        global = true,
        help_heading = "Global options"
    )]
    pub(crate) output: Option<OutputFormat>,

    /// When the `table`, `wide` and `name` formats use colour: `auto` on a
    /// terminal, unless NO_COLOR is set.
    #[arg(
        long,
        value_name = "WHEN",
        default_value = "auto",
        global = true,
        help_heading = "Global options"
    )]
    pub(crate) color: ColorWhen,

    /// Print `table` and `wide` output taller than the terminal as it is,
    /// instead of through ORIGINCLOUD_PAGER, PAGER or `less`.
    #[arg(long, global = true, help_heading = "Global options")]
    pub(crate) no_pager: bool,

    #[command(subcommand)]
    pub(crate) command: Option<Commands>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum Commands {
    /// Authenticate with Origin Cloud Identity.
    #[command(subcommand)]
    Id(IdentityCommands),

    /// IAM Service.
    #[command(subcommand)]
    Iam(IamCommands),

    /// Serve Origin Cloud's API to an AI agent over MCP.
    #[command(subcommand)]
    Mcp(McpCommands),

    /// The groups generated from the API definitions, one per service, such
    /// as `location` and `pvs`.
    #[command(flatten)]
    Services(ServiceCommands),

    /// Cli configuration.
    #[command(subcommand)]
    Config(handler::config::ConfigCommands),

    /// Print the script that completes `origin`'s commands, flags and their
    /// values in a shell.
    ///
    /// Install it once, then start a new shell. bash: `origin completion
    /// bash > ~/.local/share/bash-completion/completions/origin`. zsh, with
    /// a directory of yours on `fpath`: `origin completion zsh >
    /// ~/.zfunc/_origin`. fish: `origin completion fish >
    /// ~/.config/fish/completions/origin.fish`. PowerShell: add `origin
    /// completion powershell | Out-String | Invoke-Expression` to `$PROFILE`.
    Completion {
        /// The shell to complete in.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Print version and exit.
    Version,
}

const API_URL: &str = "https://graph.origincloudapis.eu/graphql";
fn get_default_api_url() -> String {
    API_URL.to_string()
}
