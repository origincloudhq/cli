//! `origin skill install`, `list` and `update`, run as a user would, in a
//! home directory and a project of their own: never the developer's own
//! agents' directories.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Every agent that reads skills, and its skill directory at the user scope
/// (under the home directory) and the project scope (under the project).
const AGENTS: &[(&str, &str, &str)] = &[
    ("claude-code", ".claude/skills", ".claude/skills"),
    ("codex", ".agents/skills", ".agents/skills"),
    ("cursor", ".cursor/skills", ".agents/skills"),
    ("gemini-cli", ".gemini/skills", ".agents/skills"),
    ("github-copilot", ".copilot/skills", ".agents/skills"),
];

/// The skills the CLI carries, as scripts/generate.sh copied them.
const SKILLS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/skills");

const ACCESS_TOKEN: &str = "eyJhbGciOiJub25lIn0.access-token-value.sig";
const CLIENT_SECRET: &str = "client-secret-value-0123456789";
const REFRESH_TOKEN: &str = "refresh-token-value-9876543210";

/// A home directory with a login, a project directory, and an empty
/// directory standing in for `PATH`.
struct Machine {
    _root: tempfile::TempDir,
    home: PathBuf,
    project: PathBuf,
    bin: PathBuf,
}

impl Machine {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let project = root.path().join("project");
        let bin = root.path().join("bin");
        for dir in [&home, &project, &bin] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let data = home.join(".local/share/origincloud");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(
            data.join("session.json"),
            format!(r#"{{"access_token":"{ACCESS_TOKEN}","refresh_token":"{REFRESH_TOKEN}"}}"#),
        )
        .unwrap();
        Self {
            _root: root,
            home,
            project,
            bin,
        }
    }

    /// `origin -o json {args}` on this machine, with credentials in the
    /// environment too.
    fn origin(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_origin"))
            .args(["-o", "json"])
            .args(args)
            .current_dir(&self.project)
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", &self.bin)
            .env("ORIGINCLOUD_ACCESS_TOKEN", ACCESS_TOKEN)
            .env("ORIGINCLOUD_CLIENT_ID", "client-id")
            .env("ORIGINCLOUD_CLIENT_SECRET", CLIENT_SECRET)
            .output()
            .expect("origin runs")
    }

    /// The list `origin skill {args}` prints, failing the test when it
    /// fails.
    fn skill(&self, args: &[&str]) -> Vec<Value> {
        let output = self.origin(&[&["skill"], args].concat());
        assert!(
            output.status.success(),
            "origin skill {args:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .clone()
    }

    /// `origin skill {args}`'s failure, failing the test when it succeeds.
    fn refused(&self, args: &[&str]) -> String {
        let output = self.origin(&[&["skill"], args].concat());
        assert!(!output.status.success(), "origin skill {args:?} succeeded");
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }

    /// Every file under the home and the project directories but the
    /// login, with its content.
    fn files(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        for dir in [&self.home, &self.project] {
            walk(dir, &mut files);
        }
        files.retain(|(path, _)| !path.ends_with("session.json"));
        files.sort();
        files
    }
}

fn walk(dir: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if std::fs::symlink_metadata(&path).unwrap().is_dir() {
            walk(&path, files);
        } else if path.is_file() {
            let content = std::fs::read(&path).unwrap();
            files.push((path, content));
        }
    }
}

/// The files of `dir`, by their paths in it.
fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    walk(dir, &mut files);
    let mut files: Vec<(String, Vec<u8>)> = files
        .into_iter()
        .map(|(path, content)| {
            let relative = path.strip_prefix(dir).unwrap().to_str().unwrap().to_owned();
            (relative, content)
        })
        .collect();
    files.sort();
    files
}

/// `text` without the lines of `origin`'s marker.
fn unmarked(text: &str) -> String {
    text.split_inclusive('\n')
        .filter(|l| !l.trim_start().starts_with("origin-skills-"))
        .collect()
}

/// The marker's value of `key` in `text`.
fn marker(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|l| l.trim_start().strip_prefix(&format!("{key}: ")))
        .unwrap_or_else(|| panic!("no {key} in {text}"))
        .trim_matches('"')
        .to_owned()
}

/// The digest as `origin` computes it, which this pins.
fn digest(files: &[(String, Vec<u8>)]) -> String {
    let mut files = files.to_vec();
    files.sort();
    let mut hash = Sha256::new();
    for (path, content) in &files {
        hash.update(path.as_bytes());
        hash.update([0]);
        hash.update(content.len().to_string().as_bytes());
        hash.update([0]);
        hash.update(content);
    }
    let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// Asserts the installed `dir` is the skill `name` as the CLI carries it,
/// with its marker.
fn assert_is_the_skill(dir: &Path, name: &str) {
    let installed = tree(dir);
    let carried = tree(&Path::new(SKILLS).join(name));
    let paths = |files: &[(String, Vec<u8>)]| files.iter().map(|f| f.0.clone()).collect::<Vec<_>>();
    assert_eq!(paths(&installed), paths(&carried), "{}", dir.display());
    for ((path, content), (_, expected)) in installed.iter().zip(&carried) {
        if path == "SKILL.md" {
            let text = String::from_utf8(content.clone()).unwrap();
            assert_eq!(unmarked(&text).as_bytes(), expected.as_slice());
            assert_eq!(marker(&text, "origin-skills-digest"), digest(&carried));
            assert_eq!(
                marker(&text, "origin-skills-revision"),
                include_str!("../../../SKILLS_REVISION").trim()
            );
        } else {
            assert_eq!(content, expected, "{path}");
        }
    }
}

/// Makes the installed `dir` look as an older version `origin` installed
/// would: other content, and a marker that names it.
fn age(dir: &Path) {
    let skill_md = dir.join("SKILL.md");
    let text = unmarked(&std::fs::read_to_string(&skill_md).unwrap());
    let older = format!("{text}\nAn older version.\n");
    std::fs::write(&skill_md, &older).unwrap();
    let files = tree(dir);
    let marked = older.replacen(
        "metadata:\n",
        &format!(
            "metadata:\n  origin-skills-revision: \"{}\"\n  origin-skills-digest: \"{}\"\n",
            "0".repeat(40),
            digest(&files)
        ),
        1,
    );
    std::fs::write(&skill_md, marked).unwrap();
}

fn results(outcomes: &[Value]) -> Vec<&str> {
    outcomes
        .iter()
        .map(|o| o["result"].as_str().unwrap())
        .collect()
}

fn status(rows: &[Value], name: &str, path: &Path) -> String {
    rows.iter()
        .find(|r| r["name"] == name && r["path"] == path.display().to_string())
        .unwrap_or_else(|| panic!("no {name} at {} in {rows:?}", path.display()))["status"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn each_agent_and_scope_installs_once_lists_and_updates() {
    for (agent, user, project) in AGENTS {
        for (scope, dir) in [("user", *user), ("project", *project)] {
            let machine = Machine::new();
            let base = if scope == "user" {
                &machine.home
            } else {
                &machine.project
            };
            let target = base.join(dir).join("origin");
            let args = ["install", "--agent", agent, "--scope", scope];

            let first = machine.skill(&args);
            assert_eq!(results(&first), ["added"], "{agent} {scope}");
            assert_eq!(first[0]["path"], target.display().to_string());
            assert_eq!(first[0]["agents"][0], *agent);
            assert_is_the_skill(&target, "origin");
            let installed = tree(&target);

            assert_eq!(
                results(&machine.skill(&args)),
                ["unchanged"],
                "{agent} {scope}"
            );
            assert_eq!(tree(&target), installed, "{agent} {scope}");

            let rows = machine.skill(&["list"]);
            assert_eq!(status(&rows, "origin", &target), "current");
            assert!(rows
                .iter()
                .any(|r| r["name"] == "origincloud-sdk" && r["status"] == "not-installed"));

            age(&target);
            let rows = machine.skill(&["list", "--agent", agent, "--scope", scope]);
            assert_eq!(status(&rows, "origin", &target), "outdated");
            assert_eq!(
                rows.iter().find(|r| r["status"] == "outdated").unwrap()["installed"],
                "000000000000"
            );
            let updated = machine.skill(&["update"]);
            assert_eq!(results(&updated), ["updated"], "{agent} {scope}");
            assert_is_the_skill(&target, "origin");
            assert_eq!(results(&machine.skill(&["update"])), ["unchanged"]);
            assert_eq!(
                status(&machine.skill(&["list"]), "origin", &target),
                "current"
            );
        }
    }
}

/// The agents that read `.agents/skills` at the project scope share one
/// copy there.
#[test]
fn the_agents_reading_agents_skills_share_one_copy() {
    let machine = Machine::new();
    let outcomes = machine.skill(&[
        "install",
        "--agent",
        "claude-code,codex,cursor,gemini-cli,github-copilot",
        "--scope",
        "project",
    ]);
    assert_eq!(results(&outcomes), ["added", "added"]);
    assert_eq!(
        outcomes[1]["agents"],
        serde_json::json!(["codex", "cursor", "gemini-cli", "github-copilot"])
    );
    assert_eq!(
        outcomes[1]["path"],
        machine
            .project
            .join(".agents/skills/origin")
            .display()
            .to_string()
    );
}

#[test]
fn all_installs_every_skill_the_cli_carries() {
    let machine = Machine::new();
    let outcomes = machine.skill(&["install", "--all", "--agent", "codex"]);
    let mut names: Vec<&str> = outcomes
        .iter()
        .map(|o| o["name"].as_str().unwrap())
        .collect();
    names.sort();
    let mut carried: Vec<String> = std::fs::read_dir(SKILLS)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    carried.sort();
    assert_eq!(names, carried);
    assert!(names.contains(&"origincloud-terraform"));
    for name in &carried {
        assert_is_the_skill(&machine.home.join(".agents/skills").join(name), name);
    }
    // Named, one only; and a name it doesn't carry is refused.
    let machine = Machine::new();
    let outcomes = machine.skill(&["install", "origincloud-sdk", "--agent", "codex"]);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0]["name"], "origincloud-sdk");
    machine.refused(&["install", "no-such-skill", "--agent", "codex"]);
    machine.refused(&["install", "origin", "--all", "--agent", "codex"]);
}

#[test]
fn dir_installs_into_a_directory_of_your_own_and_is_remembered() {
    let machine = Machine::new();
    let outcomes = machine.skill(&["install", "--dir", "custom"]);
    let target = machine.project.join("custom/origin");
    assert_eq!(outcomes[0]["path"], target.display().to_string());
    assert!(outcomes[0]["scope"].is_null());
    assert_is_the_skill(&target, "origin");
    assert_eq!(
        status(
            &machine.skill(&["list", "--dir", "custom"]),
            "origin",
            &target
        ),
        "current"
    );
    // Remembered: listed and updated from another directory too.
    let elsewhere = Machine {
        project: machine.project.join("sub"),
        ..Machine::new()
    };
    std::fs::create_dir_all(&elsewhere.project).unwrap();
    let elsewhere = Machine {
        home: machine.home.clone(),
        ..elsewhere
    };
    assert_eq!(
        status(&elsewhere.skill(&["list"]), "origin", &target),
        "current"
    );
    age(&target);
    assert_eq!(results(&elsewhere.skill(&["update"])), ["updated"]);
    assert_is_the_skill(&target, "origin");
    // But not by an agent: it is no agent's directory.
    assert!(elsewhere.skill(&["update", "--agent", "codex"]).is_empty());
    // --dir and --agent are one or the other.
    machine.refused(&["install", "--dir", "custom", "--agent", "codex"]);
}

/// A skill directory `origin` didn't write, or one changed since, is left
/// as it is, and nothing else is installed by that command, unless
/// `--force`.
#[test]
fn someone_elses_skill_is_refused_and_force_replaces_it() {
    let machine = Machine::new();
    let theirs = machine.home.join(".claude/skills/origin");
    std::fs::create_dir_all(&theirs).unwrap();
    let hand_made = "---\nname: origin\ndescription: Mine.\n---\nMine.\n";
    std::fs::write(theirs.join("SKILL.md"), hand_made).unwrap();

    let said = machine.refused(&["install", "--agent", "codex,claude-code"]);
    assert!(
        said.contains("wasn't installed by `origin`") && said.contains("--force"),
        "{said}"
    );
    assert_eq!(
        std::fs::read_to_string(theirs.join("SKILL.md")).unwrap(),
        hand_made
    );
    assert!(
        !machine.home.join(".agents").exists(),
        "codex's was left too"
    );

    let rows = machine.skill(&["list"]);
    assert_eq!(status(&rows, "origin", &theirs), "unmanaged");
    let updated = machine.skill(&["update"]);
    assert_eq!(results(&updated), ["skipped"]);
    assert_eq!(
        std::fs::read_to_string(theirs.join("SKILL.md")).unwrap(),
        hand_made
    );

    let forced = machine.skill(&["install", "--agent", "codex,claude-code", "--force"]);
    assert_eq!(results(&forced), ["added", "replaced"]);
    assert_is_the_skill(&theirs, "origin");

    // Changed since: a reference edited.
    let reference = theirs.join("references/pvs.md");
    std::fs::write(&reference, "edited\n").unwrap();
    assert_eq!(
        status(&machine.skill(&["list"]), "origin", &theirs),
        "modified"
    );
    let said = machine.refused(&["install", "--agent", "claude-code"]);
    assert!(said.contains("was changed since"), "{said}");
    let updated = machine.skill(&["update", "--agent", "claude-code"]);
    assert_eq!(results(&updated), ["skipped"]);
    assert_eq!(std::fs::read_to_string(&reference).unwrap(), "edited\n");
    // A file added is a change too.
    machine.skill(&["install", "--agent", "claude-code", "--force"]);
    std::fs::write(theirs.join("notes.md"), "mine\n").unwrap();
    assert_eq!(
        status(&machine.skill(&["list"]), "origin", &theirs),
        "modified"
    );
    machine.skill(&["install", "--agent", "claude-code", "--force"]);
    assert_is_the_skill(&theirs, "origin");
}

#[test]
fn the_other_skills_in_the_directory_are_untouched() {
    let machine = Machine::new();
    let skills = machine.home.join(".claude/skills");
    let mine = skills.join("mine");
    std::fs::create_dir_all(mine.join("references")).unwrap();
    std::fs::write(
        mine.join("SKILL.md"),
        "---\nname: mine\ndescription: m\n---\n",
    )
    .unwrap();
    std::fs::write(mine.join("references/a.md"), "a\n").unwrap();
    std::fs::write(skills.join("README"), "not a skill\n").unwrap();
    let before = tree(&skills);

    machine.skill(&["install", "--all", "--agent", "claude-code"]);
    age(&skills.join("origin"));
    machine.skill(&["update"]);
    machine.skill(&["install", "--all", "--agent", "claude-code", "--force"]);
    let after = tree(&skills);
    for file in &before {
        assert!(after.contains(file), "{} changed", file.0);
    }
    // Nothing left beside them: no temporary or old copy.
    let mut names: Vec<String> = std::fs::read_dir(&skills)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "README",
            "mine",
            "origin",
            "origincloud-sdk",
            "origincloud-terraform"
        ]
    );
}

/// A skill directory kept as a symbolic link, such as into a dotfiles
/// repository, stays one, and its target is written; so does a linked
/// skills directory.
#[cfg(unix)]
#[test]
fn a_linked_skill_directory_stays_a_link() {
    let machine = Machine::new();
    let dotfiles = machine.home.join("dotfiles");
    std::fs::create_dir_all(dotfiles.join("origin")).unwrap();
    let skills = machine.home.join(".claude/skills");
    std::fs::create_dir_all(&skills).unwrap();
    let link = skills.join("origin");
    std::os::unix::fs::symlink("../../dotfiles/origin", &link).unwrap();

    // An empty directory isn't `origin`'s, so it takes --force.
    machine.refused(&["install", "--agent", "claude-code"]);
    let outcomes = machine.skill(&["install", "--agent", "claude-code", "--force"]);
    assert_eq!(results(&outcomes), ["replaced"]);
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        Path::new("../../dotfiles/origin")
    );
    assert_is_the_skill(&dotfiles.join("origin"), "origin");
    age(&dotfiles.join("origin"));
    assert_eq!(results(&machine.skill(&["update"])), ["updated"]);
    assert!(std::fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_is_the_skill(&dotfiles.join("origin"), "origin");
    assert_eq!(std::fs::read_dir(&skills).unwrap().count(), 1);
    assert_eq!(std::fs::read_dir(&dotfiles).unwrap().count(), 1);

    // The whole skills directory linked.
    let shared = machine.home.join("shared-skills");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::create_dir_all(machine.home.join(".cursor")).unwrap();
    std::os::unix::fs::symlink(&shared, machine.home.join(".cursor/skills")).unwrap();
    machine.skill(&["install", "--agent", "cursor"]);
    assert!(
        std::fs::symlink_metadata(machine.home.join(".cursor/skills"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_is_the_skill(&shared.join("origin"), "origin");
}

/// An update keeps the permissions of the files and the directory it
/// replaces.
#[cfg(unix)]
#[test]
fn an_update_keeps_the_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    let machine = Machine::new();
    machine.skill(&["install", "--agent", "codex"]);
    let target = machine.home.join(".agents/skills/origin");
    age(&target);
    std::fs::set_permissions(
        target.join("SKILL.md"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(results(&machine.skill(&["update"])), ["updated"]);
    assert_eq!(mode(&target.join("SKILL.md")), 0o600);
    assert_eq!(mode(&target), 0o700);
}

/// Whatever the login and the environment hold, no file `origin skill
/// install` writes holds any of it.
#[test]
fn no_file_written_holds_a_credential() {
    let machine = Machine::new();
    for (agent, _, _) in AGENTS {
        for scope in ["user", "project"] {
            machine.skill(&["install", "--all", "--agent", agent, "--scope", scope]);
        }
    }
    machine.skill(&["install", "--all", "--dir", "custom"]);
    let files = machine.files();
    assert!(files.len() > 50, "{}", files.len());
    for (path, content) in files {
        let text = String::from_utf8_lossy(&content);
        for secret in [ACCESS_TOKEN, REFRESH_TOKEN, CLIENT_SECRET, "client-id"] {
            assert!(
                !text.contains(secret),
                "{} holds {secret:?}",
                path.display()
            );
        }
    }
}

#[test]
fn without_a_terminal_the_agent_must_be_named() {
    let machine = Machine::new();
    let said = machine.refused(&["install"]);
    assert!(
        said.contains("--agent") && said.contains("claude-code") && said.contains("github-copilot"),
        "{said}"
    );
    assert!(!said.contains("claude-desktop"), "{said}");
    // An agent without skills isn't a value.
    machine.refused(&["install", "--agent", "claude-desktop"]);
    assert!(machine.files().is_empty(), "{:?}", machine.files());
}

/// The notice of outdated skills is for a terminal: a command whose output
/// goes to a pipe never prints it, nor notes it as said.
#[test]
fn no_notice_without_a_terminal() {
    let machine = Machine::new();
    machine.skill(&["install", "--agent", "codex"]);
    age(&machine.home.join(".agents/skills/origin"));
    let json = machine.origin(&["version"]);
    let table = Command::new(env!("CARGO_BIN_EXE_origin"))
        .args(["-o", "table", "version"])
        .env_clear()
        .env("HOME", &machine.home)
        .output()
        .unwrap();
    for output in [json, table] {
        assert!(output.status.success());
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let state =
        std::fs::read_to_string(machine.home.join(".local/state/origincloud/skills.json")).unwrap();
    assert!(!state.contains("noticed"), "{state}");
}
