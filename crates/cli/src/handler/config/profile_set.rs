use crate::config::{ClientProfile, OutputFormat};
use crate::identity::{is_region, is_resource_id, RESOURCE_ID_RULE};
use crate::output::{CommandResult, SerializableOutput};
use anyhow::{anyhow, bail};
use std::str::FromStr;

const VALID_KEYS: [&str; 5] = [
    "default-username",
    "output-format",
    "default-project",
    "default-zone",
    "region",
];

/// Sets `key` to `value` in `profile`, which is the profile as its file holds
/// it (no environment overrides), and saves it.
pub(super) fn perform_profile_set(
    mut profile: ClientProfile,
    key: &str,
    value: String,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let id = |what: &str| {
        if is_resource_id(&value) {
            Ok(Some(value.clone()))
        } else {
            Err(anyhow!("{value:?} isn't a {what}: {RESOURCE_ID_RULE}"))
        }
    };
    match key {
        "default-username" => profile.default_username = Some(value),
        "output-format" => {
            let output_format = OutputFormat::from_str(value.as_str())
                .map_err(|_| anyhow!("invalid output format"))?;
            profile.output_format = Some(output_format)
        }
        "default-project" => profile.default_project = id("project id")?,
        "default-zone" => profile.default_zone = id("zone, such as eu-central1-az1")?,
        "region" => {
            if !is_region(&value) {
                bail!("{value:?} isn't a region, such as eu-central1: expected lowercase letters, digits and `-`");
            }
            profile.region = Some(value)
        }
        other => {
            return Ok(Box::new(CommandResult::Failure(format!(
                "invalid profile key '{other}'. valid values: {}",
                VALID_KEYS.join(", ")
            ))))
        }
    }

    profile
        .safe_to_file()
        .map_err(|e| anyhow!("failed to save profile: {e}"))?;

    Ok(Box::new(CommandResult::Success))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ENV_LOCK;

    /// A variable set in the shell overrides the profile while the CLI runs,
    /// but `profile-set` changes the file alone: the variable isn't saved.
    #[test]
    fn the_environment_isnt_saved_into_the_file() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("profile.json");
        std::env::set_var("ORIGINCLOUD_ZONE", "eu-central1-az9");
        std::env::set_var("ORIGINCLOUD_REGION", "eu-central9");

        perform_profile_set(
            ClientProfile::load_file(Some(&file)),
            "default-project",
            "my-project-123".to_owned(),
        )
        .unwrap();

        std::env::remove_var("ORIGINCLOUD_ZONE");
        std::env::remove_var("ORIGINCLOUD_REGION");
        let saved = std::fs::read_to_string(&file).unwrap();
        assert!(saved.contains("my-project-123"), "{saved}");
        assert!(
            !saved.contains("az9") && !saved.contains("central9"),
            "{saved}"
        );
    }

    #[test]
    fn values_are_checked_before_anything_is_saved() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("profile.json");
        for (key, value) in [
            ("region", "EU/central1"),
            ("region", "eu.central1"),
            ("default-project", "a/b"),
            ("default-project", ".."),
            ("default-zone", ""),
        ] {
            assert!(
                perform_profile_set(ClientProfile::load_file(Some(&file)), key, value.to_owned())
                    .is_err(),
                "{key} {value:?}"
            );
        }
        assert!(!file.exists());
        perform_profile_set(
            ClientProfile::load_file(Some(&file)),
            "region",
            "eu-central1".to_owned(),
        )
        .unwrap();
        assert!(file.exists());
    }
}
