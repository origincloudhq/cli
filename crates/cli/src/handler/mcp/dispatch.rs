use origincloud_mcp::{ClientSource, Defaults, Options, Server};
use rmcp::ServiceExt;

use super::commands::{McpCommands, ServeArgs};
use crate::config::ClientProfile;

/// Runs `cmd`. Standard output is the MCP session's, so nothing is printed
/// on it: an error is the caller's to report on standard error.
pub async fn run_mcp_command(profile: &ClientProfile, cmd: McpCommands) -> anyhow::Result<()> {
    match cmd {
        McpCommands::Serve(args) => serve(profile, args).await,
    }
}

async fn serve(profile: &ClientProfile, args: ServeArgs) -> anyhow::Result<()> {
    let options = Options {
        toolsets: (!args.toolsets.is_empty()).then_some(args.toolsets),
        read_only: args.read_only,
        defaults: Defaults {
            project: profile.default_project.clone(),
            zone: profile.default_zone.clone(),
            // The region the tools' calls go to, which is also the one the
            // commands complete regional names with.
            region: profile.region_in_effect().map(|(region, _)| region),
        },
    };
    let server = Server::new(ProfileClients(profile.clone()), options)?;
    let session = server.serve(rmcp::transport::stdio()).await?;
    session.waiting().await?;
    Ok(())
}

/// The SDK clients of the profile, as the generated commands make theirs.
struct ProfileClients(ClientProfile);

impl ClientSource for ProfileClients {
    fn client(&self, service: &str) -> Result<origincloud_core::Client, origincloud_core::Error> {
        crate::sdk::mcp_client(&self.0, service)
            .map_err(|e| origincloud_core::Error::Config(format!("{e:#}")))
    }
}
