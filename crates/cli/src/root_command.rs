use std::path::PathBuf;

use crate::config::OutputFormat;
use crate::handler;
use crate::handler::admin::AdministrationCommands;
use crate::handler::compute::ComputeCommands;
use crate::handler::iam::IamCommands;
use crate::handler::id::IdentityCommands;
use crate::handler::network::NetworkCommands;
use crate::handler::storage::StorageCommands;
use crate::pvs::PvsCommands;
use clap::{Args, Parser, Subcommand};

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

    /// Output format. Overrides the profile's `output-format`.
    #[arg(
        long,
        value_name = "FORMAT",
        global = true,
        help_heading = "Global options"
    )]
    pub(crate) output: Option<OutputFormat>,

    #[command(subcommand)]
    pub(crate) command: Option<Commands>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum Commands {
    /// Authenticate with Origin Cloud Identity.
    #[command(subcommand)]
    Id(IdentityCommands),

    /// Administration Service.
    #[command(subcommand)]
    Admin(AdministrationCommands),

    /// Compute Service.
    #[command(subcommand)]
    Compute(ComputeCommands),

    /// IAM Service.
    #[command(subcommand)]
    Iam(IamCommands),

    /// Network Service.
    #[command(subcommand)]
    Network(NetworkCommands),

    /// Persistent volume service: volumes, snapshots and storage classes.
    ///
    /// Generated from the API definitions. Resource names are given as their
    /// parts (`--project`, `--zone`, `--volume`), or the resource's flag
    /// takes a whole name; `--project` and `--zone` default to the profile's
    /// `default-project` and `default-zone`.
    Pvs(PvsCli),

    /// Tag changes by key on volumes and snapshots, keeping their other tags.
    #[command(subcommand)]
    Storage(StorageCommands),

    /// Cli configuration.
    #[command(subcommand)]
    Config(handler::config::ConfigCommands),

    /// Print version and exit.
    Version,
}

const API_URL: &str = "https://graph.origincloudapis.eu/graphql";
fn get_default_api_url() -> String {
    API_URL.to_string()
}

/// `origin pvs`: its options, and the command.
#[derive(Args)]
pub(crate) struct PvsCli {
    /// Base URL the calls go to instead of the service's own host
    /// (`https://pvs.origincloudapis.eu`, or the profile's region's), such as
    /// a test server's. It must be `https://`, or `http://` to this machine
    /// (`localhost`, `127.0.0.1`, `[::1]`).
    #[arg(
        long,
        value_name = "URL",
        env = "ORIGINCLOUD_ENDPOINT",
        global = true,
        help_heading = "Global options"
    )]
    pub(crate) endpoint: Option<String>,

    #[command(subcommand)]
    pub(crate) command: PvsCommands,
}
