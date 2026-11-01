use super::commands::IdentityCommands;
use crate::config::ClientProfile;
use crate::handler::id::login::perform_login;
use crate::handler::id::token::perform_token;
use crate::output::SerializableOutput;
use url::Url;

pub async fn run_identity_command(
    profile: &ClientProfile,
    api_url: &Url,
    command: IdentityCommands,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    match command {
        IdentityCommands::Login { device } => {
            Ok(perform_login(device, profile.default_username.clone(), api_url).await?)
        }
        IdentityCommands::Token { as_header, raw } => Ok(perform_token(as_header, raw).await?),
    }
}
