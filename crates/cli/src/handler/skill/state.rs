//! What `origin` keeps of the skills it installed, in its state directory
//! (`$XDG_STATE_HOME/origincloud/skills.json`, by default under
//! `~/.local/state`): where it wrote each copy, so that `origin skill list`
//! and `update` find the copies in other projects and in `--dir`s too, and
//! the version of `origin` that last looked for outdated ones.
//!
//! That look is the notice an upgrade of `origin` brings ([`notice`]):
//! once per version of `origin`, on a terminal, it reads the copies the file
//! names, and says on standard error which are outdated. Every other run
//! reads the file alone, which is small, and stops at the version in it:
//! no skill directory is read, let alone walked, on every command. A run
//! without the file, before any skill was installed, reads nothing else.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use super::embedded::{self, short, REVISION};
use super::install::{inspect, Copy};
use crate::handler::agents::Host;
use crate::handler::files::create;

/// The directory under the state directory, as the configuration and the
/// login have theirs under theirs.
const VENDOR: &str = "origincloud";
const FILE: &str = "skills.json";

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct State {
    /// The copies `origin` wrote, and where.
    #[serde(default)]
    pub installed: Vec<Record>,
    /// The version of `origin`, and of its skills, that last looked for
    /// outdated copies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noticed: Option<String>,
}

/// A copy of a skill `origin` wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    /// The skill's directory.
    pub path: PathBuf,
    /// The agents that read it; none for a `--dir`.
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Where the state is kept on `host`.
pub(crate) fn path(host: &Host) -> PathBuf {
    host.var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| host.home.join(".local/state"))
        .join(VENDOR)
        .join(FILE)
}

/// The state, empty when there is none or it doesn't parse: it only ever
/// saves looking.
pub(crate) fn load(host: &Host) -> State {
    read(&path(host)).unwrap_or_default()
}

fn read(path: &Path) -> Option<State> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn save(host: &Host, state: &State) -> anyhow::Result<()> {
    let path = path(host);
    let dir = path.parent().expect("under the state directory");
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let temporary = dir.join(format!("{FILE}.tmp"));
    let mut text = serde_json::to_string_pretty(state)?;
    text.push('\n');
    create(&temporary, text, None).with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, &path).with_context(|| format!("writing {}", path.display()))
}

/// Notes `records`, replacing those of the same paths.
pub(crate) fn remember(host: &Host, records: Vec<Record>) -> anyhow::Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    let mut state = load(host);
    for record in records {
        state.installed.retain(|r| r.path != record.path);
        state.installed.push(record);
    }
    save(host, &state)
}

/// Says on standard error which copies `origin` installed are older than
/// the skills it carries, the first time this version of `origin` runs a
/// command; the caller runs it only on a terminal and never for `json`.
/// Whatever fails, the command runs on as if there was nothing to say.
pub(crate) fn notice() {
    let Ok(host) = Host::current() else {
        return;
    };
    if let Some(text) = outdated(&host, env!("CARGO_PKG_VERSION")) {
        eprint!("{text}");
    }
}

/// What the notice says on `host` for `version` of `origin`, if anything,
/// and the state noted so that it is said once: copies gone, or no longer
/// `origin`'s, are forgotten.
pub(crate) fn outdated(host: &Host, version: &str) -> Option<String> {
    let key = format!("{version}+{}", short(REVISION));
    let mut state = read(&path(host))?;
    if state.noticed.as_deref() == Some(key.as_str()) {
        return None;
    }
    let mut old = Vec::new();
    state.installed.retain(|record| {
        let Some(skill) = record
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(embedded::skill)
        else {
            return false;
        };
        match inspect(&record.path, &skill) {
            Ok(Some(Copy::Outdated(marker))) => {
                old.push((record.path.clone(), marker.revision));
                true
            }
            Ok(Some(Copy::Current(_) | Copy::Modified(_))) | Err(_) => true,
            Ok(None | Some(Copy::Unmanaged)) => false,
        }
    });
    state.noticed = Some(key);
    let _ = save(host, &state);
    if old.is_empty() {
        return None;
    }
    let mut text = format!(
        "note: this origin carries newer agent skills (origincloudhq/skills@{}) than these:\n",
        short(REVISION)
    );
    for (path, revision) in old {
        text.push_str(&format!("  {} ({})\n", path.display(), short(&revision)));
    }
    text.push_str("Run `origin skill update` to update them.\n");
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::agents::Os;
    use crate::handler::skill::marker;

    /// Writes the `origin` skill to `dir` as an older version `origin`
    /// installed would be: other content, and a marker naming it.
    fn older(dir: &Path) {
        let skill = embedded::skill("origin").unwrap();
        let mut files: Vec<(String, Vec<u8>)> = skill
            .files
            .iter()
            .map(|(p, c)| ((*p).to_owned(), c.to_vec()))
            .collect();
        let at = files.iter().position(|(p, _)| p == "SKILL.md").unwrap();
        files[at].1.extend_from_slice(b"\nAn older version.\n");
        let digest = marker::digest(files.iter().map(|(p, c)| (p.as_str(), c.as_slice())));
        let text = String::from_utf8(files[at].1.clone()).unwrap();
        files[at].1 = marker::mark(&text, &"0".repeat(40), &digest)
            .unwrap()
            .into_bytes();
        for (path, content) in files {
            let file = dir.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, content).unwrap();
        }
    }

    #[test]
    fn the_notice_names_the_outdated_copies_once_per_version() {
        let root = tempfile::tempdir().unwrap();
        let host = Host {
            home: root.path().join("home"),
            cwd: root.path().join("project"),
            os: Os::current(),
            vars: Vec::new(),
        };
        // Nothing installed: nothing read, nothing said, nothing written.
        assert_eq!(outdated(&host, "1.0.0"), None);
        assert!(!path(&host).exists());

        let dir = host.home.join(".claude/skills/origin");
        older(&dir);
        remember(
            &host,
            vec![Record {
                path: dir.clone(),
                agents: vec!["claude-code".to_owned()],
                scope: Some("user".to_owned()),
            }],
        )
        .unwrap();
        let said = outdated(&host, "1.0.0").expect("a notice");
        assert!(said.contains(&dir.display().to_string()), "{said}");
        assert!(said.contains("origin skill update"), "{said}");
        // Once.
        assert_eq!(outdated(&host, "1.0.0"), None);
        // Again for the next version, while it is still outdated.
        assert!(outdated(&host, "1.0.1").is_some());

        // A copy gone is forgotten.
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(outdated(&host, "1.0.2"), None);
        assert!(load(&host).installed.is_empty());
    }

    #[test]
    fn the_state_is_under_xdg_state_home() {
        let host = Host {
            home: PathBuf::from("/home/u"),
            cwd: PathBuf::from("/work"),
            os: Os::Unix,
            vars: vec![("XDG_STATE_HOME".to_owned(), "/state".into())],
        };
        assert_eq!(path(&host), Path::new("/state/origincloud/skills.json"));
        let host = Host {
            vars: Vec::new(),
            ..host
        };
        assert_eq!(
            path(&host),
            Path::new("/home/u/.local/state/origincloud/skills.json")
        );
    }
}
