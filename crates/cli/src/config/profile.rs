use super::file_io::{read_file_opt, safe_file};
use super::paths::{self, ENV_CONFIG_DIR_USER};
use crate::config::env::{env_or, expect_env, try_env};
use crate::output::{SerializableOutput, SerializationError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::str::FromStr;

const FILE_NAME_PROFILE: &str = "default-profile.json";

/// Environment overrides for the saved profile.
const ENV_USERNAME: &str = "ORIGINCLOUD_USERNAME";
const ENV_OUTPUT_FORMAT: &str = "ORIGINCLOUD_OUTPUT_FORMAT";
/// The project and zone the generated commands, such as `origin pvs`, use
/// when `--project` or `--zone` isn't given.
const ENV_PROJECT: &str = "ORIGINCLOUD_PROJECT";
const ENV_ZONE: &str = "ORIGINCLOUD_ZONE";
/// The region whose front door the generated commands of a regional
/// service call; the same variable the SDKs read.
const ENV_REGION: &str = "ORIGINCLOUD_REGION";

fn config_base_dir() -> String {
    env_or(
        ENV_CONFIG_DIR_USER,
        format!("{}/.config", expect_env::<String>("HOME")),
    )
}

fn profile_path() -> PathBuf {
    paths::path(config_base_dir().as_str(), FILE_NAME_PROFILE)
}

/// A `default_org` key, which earlier versions stored but never read, is
/// ignored when an old profile file is loaded. The fields added since are
/// optional, so a profile an earlier version saved still loads, and one this
/// version saves still loads in an earlier one.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct ClientProfile {
    pub default_username: Option<String>,
    pub output_format: Option<OutputFormat>,
    /// The project of the generated commands without `--project`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_project: Option<String>,
    /// The zone of the generated commands without `--zone`, such as
    /// `eu-central1-az1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_zone: Option<String>,
    /// The region whose front door the generated commands of a regional
    /// service, such as `pvs`, call,
    /// `{service}.{region}.origincloudapis.eu`; the regionless endpoint,
    /// which serves every resource, when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// The file this profile was loaded from when `--config` named one, so a
    /// save writes back to it rather than to the default location.
    #[serde(skip)]
    source: Option<PathBuf>,
}

#[derive(Debug, Default, Serialize, Deserialize, Copy, Clone)]
pub(crate) enum OutputFormat {
    #[default]
    Json,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(OutputFormat::Json),
            _ => Err(String::from("unknown output format")),
        }
    }
}

impl ClientProfile {
    fn from_env() -> Self {
        Self {
            default_username: try_env(ENV_USERNAME).ok(),
            output_format: try_env(ENV_OUTPUT_FORMAT).ok(),
            default_project: try_env(ENV_PROJECT).ok(),
            default_zone: try_env(ENV_ZONE).ok(),
            region: try_env(ENV_REGION).ok(),
            source: None,
        }
    }

    /// Loads the profile from `config_file` (the global `--config` option),
    /// or from the default location when it is `None`, then applies the
    /// environment overrides. A named file that does not exist yet loads as
    /// empty and is created by the first save.
    pub fn load(config_file: Option<&Path>) -> Self {
        let mut profile = Self::load_file(config_file);
        profile.apply(Self::from_env());
        profile
    }

    /// The profile as its file holds it, without the environment overrides:
    /// what a change to the file starts from, so that a variable set in the
    /// shell doesn't end up saved.
    pub fn load_file(config_file: Option<&Path>) -> Self {
        let path = config_file
            .map(Path::to_path_buf)
            .unwrap_or_else(profile_path);
        let mut profile = read_file_opt::<Self>(&path).unwrap_or_default();
        profile.source = config_file.map(Path::to_path_buf);
        profile
    }

    pub fn safe_to_file(&self) -> Result<(), std::io::Error> {
        match &self.source {
            Some(path) => safe_file(path, self),
            None => safe_file(&profile_path(), self),
        }
    }

    /// Overlays `other` (the environment) onto a profile loaded from file,
    /// field by field, keeping the file's value where the environment says
    /// nothing.
    ///
    /// Every field of this struct belongs in this function: one that is not
    /// overlaid is read by [`Self::from_env`] and then silently dropped,
    /// which presents as "the environment variable does nothing" rather than
    /// as a failure.
    fn apply(&mut self, other: Self) {
        if other.default_username.is_some() {
            self.default_username = other.default_username;
        }
        if other.output_format.is_some() {
            self.output_format = other.output_format;
        }
        if other.default_project.is_some() {
            self.default_project = other.default_project;
        }
        if other.default_zone.is_some() {
            self.default_zone = other.default_zone;
        }
        if other.region.is_some() {
            self.region = other.region;
        }
    }
}

impl SerializableOutput for ClientProfile {
    fn to_json(&self) -> Result<Value, SerializationError> {
        Ok(serde_json::to_value(self).expect("serialization failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::paths::{ENV_LOCK, VENDOR_PORTION};
    use super::*;
    use std::fs;

    const PROFILE: &str =
        r#"{"default_org":null,"default_username":"file-user","output_format":"Json"}"#;

    const ALL_ENV: [&str; 5] = [
        ENV_USERNAME,
        ENV_OUTPUT_FORMAT,
        ENV_PROJECT,
        ENV_ZONE,
        ENV_REGION,
    ];

    /// Points `HOME` and `XDG_CONFIG_HOME` at `dir`, so that nothing touches
    /// the developer's own `~/.config`, and clears the overrides.
    fn isolate(dir: &Path) {
        std::env::set_var("HOME", dir);
        std::env::set_var("XDG_CONFIG_HOME", dir);
        for key in ALL_ENV {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn a_saved_profile_is_read_back_from_the_default_location() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        isolate(dir.path());

        let mut profile = ClientProfile::load(None);
        assert!(profile.default_username.is_none());
        profile.default_project = Some("my-project-123".to_owned());
        profile.safe_to_file().unwrap();

        let path = dir.path().join(VENDOR_PORTION).join(FILE_NAME_PROFILE);
        assert!(path.exists());
        assert_eq!(
            ClientProfile::load(None).default_project.as_deref(),
            Some("my-project-123")
        );
    }

    /// The names are string constants, so nothing else in the build notices
    /// a change to them.
    #[test]
    fn the_profile_environment_variable_names_are_pinned() {
        assert_eq!(ENV_USERNAME, "ORIGINCLOUD_USERNAME");
        assert_eq!(ENV_OUTPUT_FORMAT, "ORIGINCLOUD_OUTPUT_FORMAT");
        assert_eq!(ENV_PROJECT, "ORIGINCLOUD_PROJECT");
        assert_eq!(ENV_ZONE, "ORIGINCLOUD_ZONE");
        assert_eq!(ENV_REGION, "ORIGINCLOUD_REGION");
    }

    /// The environment must reach the *loaded* profile, not just
    /// `from_env`: a field `apply` misses is read and discarded. Goes through
    /// `load` deliberately, since testing `from_env` alone would not notice.
    #[test]
    fn the_environment_reaches_the_loaded_profile_but_not_the_file() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        isolate(dir.path());

        std::env::set_var(ENV_USERNAME, "from-env@example.com");
        std::env::set_var(ENV_ZONE, "eu-central1-az9");
        std::env::set_var(ENV_REGION, "eu-central9");
        let profile = ClientProfile::load(None);
        assert_eq!(
            profile.default_username.as_deref(),
            Some("from-env@example.com")
        );
        assert_eq!(profile.default_zone.as_deref(), Some("eu-central1-az9"));
        assert_eq!(profile.region.as_deref(), Some("eu-central9"));

        let file = ClientProfile::load_file(None);
        assert!(file.default_username.is_none());
        assert!(file.default_zone.is_none());
        for key in ALL_ENV {
            std::env::remove_var(key);
        }
    }

    /// `--config` names the file the profile is read from and saved back
    /// to; the default location is not touched.
    #[test]
    fn a_named_config_file_is_read_and_saved_back_in_place() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        isolate(dir.path());

        let named = dir.path().join("elsewhere").join("ci-profile.json");
        fs::create_dir_all(named.parent().unwrap()).unwrap();
        fs::write(&named, PROFILE).unwrap();

        let mut profile = ClientProfile::load(Some(&named));
        assert_eq!(profile.default_username.as_deref(), Some("file-user"));

        profile.default_username = Some("ci-user".to_string());
        profile.safe_to_file().unwrap();

        assert!(fs::read_to_string(&named).unwrap().contains("ci-user"));
        assert!(
            !dir.path().join(VENDOR_PORTION).exists(),
            "saving a --config profile must not write the default location"
        );
    }
}
