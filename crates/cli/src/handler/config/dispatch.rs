use super::commands::ConfigCommands;
use crate::config::ClientProfile;
use crate::handler::config::profile_set::perform_profile_set;
use crate::output::SerializableOutput;
use std::path::Path;

/// `profile` is the profile in effect, environment overrides included;
/// `config_file` the file `--config` named, which a change is made to.
pub async fn run_config_command(
    profile: &ClientProfile,
    config_file: Option<&Path>,
    cmd: ConfigCommands,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    match cmd {
        ConfigCommands::Profile => Ok(Box::new(profile.clone())),
        ConfigCommands::ProfileSet { key, value } => Ok(perform_profile_set(
            ClientProfile::load_file(config_file),
            key.as_str(),
            value,
        )?),
    }
}
