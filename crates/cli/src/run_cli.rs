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

    match cli.command {
        None => exit(0),
        Some(Commands::Id(id_cmd)) => print_output_and_exit(
            &profile,
            handler::id::run_identity_command(&profile, &api_url, id_cmd).await,
        ),
        Some(Commands::Iam(cmd)) => print_output_and_exit(
            &profile,
            handler::iam::run_iam_command(&profile, &api_url, cmd).await,
        ),
        // The MCP session owns standard output: nothing is printed on it.
        Some(Commands::Mcp(cmd)) => match handler::mcp::run_mcp_command(&profile, cmd).await {
            Ok(()) => exit(0),
            Err(err) => {
                eprintln!("origin mcp: {err:#}");
                exit(1);
            }
        },
        Some(Commands::Services(cmd)) => {
            print_output_and_exit(&profile, crate::sdk::run(&profile, cmd).await)
        }
        Some(Commands::Config(cmd)) => print_output_and_exit(
            &profile,
            handler::config::run_config_command(&profile, cli.config.as_deref(), cmd).await,
        ),
        Some(Commands::Version) => {
            print_output_and_exit(&profile, Ok(Box::new(CliReleaseInfo::default())))
        }
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

fn print_output_and_exit(
    profile: &ClientProfile,
    result: anyhow::Result<Box<dyn SerializableOutput>>,
) -> ! {
    let output = match result {
        Ok(output) => output,
        // `err.to_string()` is a curated rendering, not a Rust `Debug` dump:
        // the GraphQL client's errors carry the server's message, stable code
        // and correlation id, and the SDK's its message, code and request id.
        Err(err) => Box::new(CommandResult::Failure(err.to_string())),
    };

    match print_output(profile, output) {
        Ok(code) => exit(code),
        Err(err) => {
            eprintln!("Failed to render response: {err}");
            exit(2);
        }
    }
}

fn print_output(
    profile: &ClientProfile,
    output: Box<dyn SerializableOutput>,
) -> anyhow::Result<i32> {
    let format = output::format_for_stdout(profile.output_format);
    let printed = output::render(format, output.as_ref(), output::width())?;
    if format == OutputFormat::Json || output.to_raw().is_some() {
        // Byte for byte as it was: nothing in it for `anstream` to strip.
        print!("{}", printed.out);
        eprint!("{}", printed.err);
    } else {
        anstream::print!("{}", printed.out);
        anstream::eprint!("{}", printed.err);
    }
    Ok(output.exit_code())
}
