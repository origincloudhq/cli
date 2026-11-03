use std::process::exit;

use crate::config::{ClientProfile, OutputFormat};
use crate::handler;
use crate::output::{self, CommandResult, SerializableOutput, SerializationError};
use crate::root_command::{Cli, Commands};
use clap::Parser;
use serde::Serialize;
use serde_json::Value;
use url::Url;

pub async fn run() -> ! {
    let cli = Cli::parse();
    cli.color.apply();
    let mut profile = ClientProfile::load(cli.config.as_deref());
    if let Some(output) = cli.output {
        profile.output_format = Some(output);
    }

    let api_url: Url = cli.api.parse().expect("failed to parse API URL");
    let printing = Printing {
        profile: &profile,
        no_pager: cli.no_pager,
    };

    match cli.command {
        None => exit(0),
        Some(Commands::Id(id_cmd)) => {
            printing.exit_with(handler::id::run_identity_command(&profile, &api_url, id_cmd).await)
        }
        Some(Commands::Iam(cmd)) => {
            printing.exit_with(handler::iam::run_iam_command(&profile, &api_url, cmd).await)
        }
        // The MCP session owns standard output: nothing is printed on it.
        Some(Commands::Mcp(cmd)) => match handler::mcp::run_mcp_command(&profile, cmd).await {
            Ok(()) => exit(0),
            Err(err) => {
                eprintln!("origin mcp: {err:#}");
                exit(1);
            }
        },
        Some(Commands::Services(cmd)) => printing.exit_with(crate::sdk::run(&profile, cmd).await),
        Some(Commands::Config(cmd)) => printing.exit_with(
            handler::config::run_config_command(&profile, cli.config.as_deref(), cmd).await,
        ),
        // The script is the output, in no format.
        Some(Commands::Completion { shell }) => {
            handler::completion::write(shell, &mut std::io::stdout());
            exit(0)
        }
        Some(Commands::Version) => printing.exit_with(Ok(Box::new(CliReleaseInfo::default()))),
    }
}

#[derive(Serialize)]
struct CliReleaseInfo {
    version: String,
}

impl Default for CliReleaseInfo {
    fn default() -> Self {
        CliReleaseInfo {
            version: String::from(env!("CARGO_PKG_VERSION")),
        }
    }
}

impl SerializableOutput for CliReleaseInfo {
    fn to_json(&self) -> Result<Value, SerializationError> {
        Ok(serde_json::to_value(self).expect("serialization failed"))
    }
}

/// How a command's output is printed: in the profile's format, through
/// the pager unless `--no-pager`.
struct Printing<'a> {
    profile: &'a ClientProfile,
    no_pager: bool,
}

impl Printing<'_> {
    fn exit_with(&self, result: anyhow::Result<Box<dyn SerializableOutput>>) -> ! {
        let output = match result {
            Ok(output) => output,
            // `err.to_string()` is a curated rendering, not a Rust `Debug`
            // dump: the GraphQL client's errors carry the server's message,
            // stable code and correlation id, and the SDK's its message, code
            // and request id.
            Err(err) => Box::new(CommandResult::Failure(err.to_string())),
        };

        match self.print(output) {
            Ok(code) => exit(code),
            Err(err) => {
                eprintln!("Failed to render response: {err}");
                exit(2);
            }
        }
    }

    fn print(&self, output: Box<dyn SerializableOutput>) -> anyhow::Result<i32> {
        let format = output::format_for_stdout(self.profile.output_format);
        let printed = output::render(format, output.as_ref(), &output::Context::current())?;
        if format == OutputFormat::Json || output.to_raw().is_some() {
            // Byte for byte as it was: nothing in it for `anstream` to strip.
            print!("{}", printed.out);
            eprint!("{}", printed.err);
        } else {
            crate::pager::print(&printed.out, self.no_pager);
            anstream::eprint!("{}", printed.err);
        }
        Ok(output.exit_code())
    }
}
