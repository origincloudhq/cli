//! `origin mcp install` and `origin mcp uninstall`, run as a user would, in
//! a home directory and a project of their own: never the developer's own
//! agents' files. The edits themselves are tested against golden files in
//! `src/handler/mcp/config_file/tests.rs`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

/// Every agent of the table, and the scopes it has.
const AGENTS: &[(&str, &[&str])] = &[
    ("claude-code", &["user", "project"]),
    ("codex", &["user", "project"]),
    ("cursor", &["user", "project"]),
    ("gemini-cli", &["user", "project"]),
    ("github-copilot", &["user", "project"]),
    ("claude-desktop", &["user"]),
];

const ACCESS_TOKEN: &str = "eyJhbGciOiJub25lIn0.access-token-value.sig";
const CLIENT_SECRET: &str = "client-secret-value-0123456789";
const REFRESH_TOKEN: &str = "refresh-token-value-9876543210";

/// A home directory with a login, a project directory, and a directory
/// for programs that stands in for `PATH`.
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
        // A login, whose tokens must never reach an agent's file.
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

    /// The outcomes `origin mcp {args}` prints, failing the test when it
    /// fails.
    fn mcp(&self, args: &[&str]) -> Vec<Value> {
        let output = self.origin(&[&["mcp"], args].concat());
        assert!(
            output.status.success(),
            "origin mcp {args:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .clone()
    }

    /// Every file under the home and the project directories but the
    /// login, with its content.
    fn files(&self) -> Vec<(PathBuf, String)> {
        let mut files = Vec::new();
        for dir in [&self.home, &self.project] {
            walk(dir, &mut files);
        }
        files.retain(|(path, _)| !path.ends_with("session.json"));
        files
    }
}

fn walk(dir: &Path, files: &mut Vec<(PathBuf, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(&path, files);
        } else {
            let text = std::fs::read_to_string(&path).unwrap();
            files.push((path, text));
        }
    }
}

fn result(outcomes: &[Value]) -> Vec<&str> {
    outcomes
        .iter()
        .map(|o| o["result"].as_str().unwrap())
        .collect()
}

#[test]
fn twice_is_one_entry_and_uninstall_removes_it() {
    for (agent, scopes) in AGENTS {
        for scope in *scopes {
            let machine = Machine::new();
            let args = ["install", "--agent", agent, "--scope", scope, "--read-only"];
            let first = machine.mcp(&args);
            assert_eq!(result(&first), ["added"], "{agent} {scope}");
            assert_eq!(first[0]["by"], "file", "{agent} {scope}: nothing on PATH");
            let file = PathBuf::from(first[0]["file"].as_str().unwrap());
            let installed = std::fs::read_to_string(&file).unwrap();
            assert_eq!(installed.matches("origincloud").count(), 1, "{installed}");
            assert!(installed.contains("--read-only"), "{installed}");

            assert_eq!(
                result(&machine.mcp(&args)),
                ["unchanged"],
                "{agent} {scope}"
            );
            assert_eq!(std::fs::read_to_string(&file).unwrap(), installed);

            let removed = machine.mcp(&["uninstall", "--agent", agent, "--scope", scope]);
            assert_eq!(result(&removed), ["removed"], "{agent} {scope}");
            assert!(!std::fs::read_to_string(&file)
                .unwrap()
                .contains("origincloud"));
            let again = machine.mcp(&["uninstall", "--agent", agent, "--scope", scope]);
            assert_eq!(result(&again), ["absent"], "{agent} {scope}");
        }
    }
}

/// The acceptance test of the rule: whatever the login and the
/// environment hold, no file `origin mcp install` writes holds any of it.
#[test]
fn no_file_written_holds_a_credential() {
    let machine = Machine::new();
    for (agent, scopes) in AGENTS {
        for scope in *scopes {
            machine.mcp(&["install", "--agent", agent, "--scope", scope]);
        }
    }
    let files = machine.files();
    assert!(files.len() >= 10, "{files:?}");
    for (path, text) in files {
        for secret in [ACCESS_TOKEN, REFRESH_TOKEN, CLIENT_SECRET, "client-id"] {
            assert!(!text.contains(secret), "{}: {text}", path.display());
        }
        let lower = text.to_lowercase();
        for word in ["token", "secret", "password", "env"] {
            assert!(
                !lower.contains(word),
                "{} has {word:?}: {text}",
                path.display()
            );
        }
    }
}

#[test]
fn print_prints_the_entry_and_changes_nothing() {
    let machine = Machine::new();
    let output = machine.origin(&[
        "mcp",
        "install",
        "--agent",
        "codex",
        "--print",
        "--toolsets",
        "pvs",
    ]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "[mcp_servers.origincloud]\ncommand = \"origin\"\nargs = [\"mcp\", \"serve\", \"--toolsets\", \"pvs\"]\n"
    );
    // Without --agent, the form most MCP clients read.
    let output = machine.origin(&["mcp", "install", "--print"]);
    assert!(output.status.success());
    let printed: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        printed["mcpServers"]["origincloud"],
        serde_json::json!({"command": "origin", "args": ["mcp", "serve"]})
    );
    assert!(machine.files().is_empty(), "{:?}", machine.files());
}

#[test]
fn without_a_terminal_the_agent_must_be_named() {
    let machine = Machine::new();
    for verb in ["install", "uninstall"] {
        let output = machine.origin(&["mcp", verb]);
        assert!(!output.status.success());
        let said = String::from_utf8_lossy(&output.stdout);
        assert!(
            said.contains("--agent") && said.contains("claude-code"),
            "{said}"
        );
    }
    assert!(machine.files().is_empty());
}

#[test]
fn claude_desktop_gets_the_absolute_path_and_has_no_project_scope() {
    let machine = Machine::new();
    machine.mcp(&["install", "--agent", "claude-desktop"]);
    let file = machine
        .home
        .join(".config/Claude/claude_desktop_config.json");
    let config: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let command = config["mcpServers"]["origincloud"]["command"]
        .as_str()
        .unwrap();
    assert!(Path::new(command).is_absolute(), "{command}");
    assert!(Path::new(command).ends_with("origin"), "{command}");

    let output = machine.origin(&[
        "mcp",
        "install",
        "--agent",
        "claude-desktop",
        "--scope",
        "project",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("no project scope"));
}

#[test]
fn a_file_that_does_not_parse_is_left_as_it_is() {
    let machine = Machine::new();
    let file = machine.project.join(".cursor/mcp.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let broken = "{ \"mcpServers\": { \"x\": ";
    std::fs::write(&file, broken).unwrap();
    for verb in ["install", "uninstall"] {
        let output = machine.origin(&["mcp", verb, "--agent", "cursor", "--scope", "project"]);
        assert!(!output.status.success());
        let said = String::from_utf8_lossy(&output.stdout);
        assert!(said.contains("isn't valid JSON"), "{said}");
    }
    assert_eq!(std::fs::read_to_string(&file).unwrap(), broken);
    assert_eq!(machine.files().len(), 1, "no copy, no temporary file");
}

#[test]
fn another_servers_entry_of_the_name_is_left_as_it_is() {
    let machine = Machine::new();
    let file = machine.home.join(".cursor/mcp.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let theirs = r#"{"mcpServers": {"origincloud": {"command": "npx", "args": ["other"]}}}"#;
    std::fs::write(&file, theirs).unwrap();
    for verb in ["install", "uninstall"] {
        let output = machine.origin(&["mcp", verb, "--agent", "cursor"]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("--name"));
    }
    assert_eq!(std::fs::read_to_string(&file).unwrap(), theirs);
    // Under another name, it is added beside it, after a copy.
    let outcomes = machine.mcp(&["install", "--agent", "cursor", "--name", "oc"]);
    assert_eq!(result(&outcomes), ["added"]);
    let backup = outcomes[0]["backup"].as_str().unwrap();
    assert_eq!(std::fs::read_to_string(backup).unwrap(), theirs);
}

/// Writes `bin/{name}`, a program that logs its arguments to
/// `bin/{name}.log` and runs `script`.
#[cfg(unix)]
fn program(machine: &Machine, name: &str, script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let log = machine.bin.join(format!("{name}.log"));
    let path = machine.bin.join(name);
    std::fs::write(
        &path,
        format!("#!/bin/sh\necho \"$*\" >> '{}'\n{script}\n", log.display()),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    log
}

/// With `claude` on PATH, its own command adds and removes the entry, and
/// the file is checked for it after.
#[cfg(unix)]
#[test]
fn claude_code_is_given_the_server_by_its_own_command() {
    let machine = Machine::new();
    let file = machine.home.join(".claude.json");
    // `mcp add --scope user NAME -- COMMAND ARGS…` writes the entry as
    // Claude Code does; `mcp remove` empties the file.
    let log = program(
        &machine,
        "claude",
        &format!(
            r#"f='{}'
case "$2" in
add) name="$5"; shift 6; c="$1"; shift; a=""
  for x in "$@"; do a="$a${{a:+,}}\"$x\""; done
  echo "{{\"mcpServers\":{{\"$name\":{{\"type\":\"stdio\",\"command\":\"$c\",\"args\":[$a],\"env\":{{}}}}}}}}" > "$f";;
remove) echo '{{}}' > "$f";;
esac"#,
            file.display()
        ),
    );
    let outcomes = machine.mcp(&["install", "--agent", "claude-code", "--read-only"]);
    assert_eq!(result(&outcomes), ["added"]);
    assert_eq!(outcomes[0]["by"], "claude mcp add");
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "mcp add --scope user origincloud -- origin mcp serve --read-only\n"
    );
    // Installed already: nothing to run.
    assert_eq!(
        result(&machine.mcp(&["install", "--agent", "claude-code", "--read-only"])),
        ["unchanged"]
    );
    // Different flags: removed, then added, as `claude mcp add` refuses a
    // name it has.
    let outcomes = machine.mcp(&["install", "--agent", "claude-code"]);
    assert_eq!(result(&outcomes), ["updated"], "{outcomes:?}");
    let outcomes = machine.mcp(&["uninstall", "--agent", "claude-code"]);
    assert_eq!(result(&outcomes), ["removed"]);
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "mcp add --scope user origincloud -- origin mcp serve --read-only\n\
         mcp remove --scope user origincloud\n\
         mcp add --scope user origincloud -- origin mcp serve\n\
         mcp remove --scope user origincloud\n"
    );
}

/// An agent's command that says it added the entry when the file the
/// table names doesn't have it fails the install: the table is wrong for
/// that agent's version, and the user is told where it looked.
#[cfg(unix)]
#[test]
fn an_agents_command_that_writes_elsewhere_fails_the_install() {
    let machine = Machine::new();
    program(&machine, "codex", "true");
    let output = machine.origin(&["mcp", "install", "--agent", "codex"]);
    assert!(!output.status.success());
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(
        said.contains("config.toml") && said.contains("doesn't have the entry"),
        "{said}"
    );
    // At the project scope, which `codex mcp add` doesn't cover, the file
    // is edited.
    let outcomes = machine.mcp(&["install", "--agent", "codex", "--scope", "project"]);
    assert_eq!(outcomes[0]["by"], "file");
}

/// A config file kept as a symbolic link, such as into a dotfiles
/// repository, stays one: the edit goes to its target, with the copy beside
/// the target, whether the link is relative or absolute.
#[cfg(unix)]
#[test]
fn a_linked_file_stays_a_link_and_its_target_is_edited() {
    let machine = Machine::new();
    let dotfiles = machine.home.join("dotfiles");
    std::fs::create_dir_all(&dotfiles).unwrap();
    let cases = [
        // (agent, the link, the target, the link's content)
        (
            "codex",
            ".codex/config.toml",
            "codex.toml",
            "../dotfiles/codex.toml".to_owned(),
        ),
        (
            "cursor",
            ".cursor/mcp.json",
            "cursor.json",
            dotfiles.join("cursor.json").display().to_string(),
        ),
    ];
    let originals = [
        "# mine\nmodel = \"gpt-5-codex\"\n",
        "{\n  \"mcpServers\": {}\n}\n",
    ];
    for ((agent, link, target, points_to), original) in cases.iter().zip(originals) {
        let link = machine.home.join(link);
        let target = dotfiles.join(target);
        std::fs::write(&target, original).unwrap();
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(points_to, &link).unwrap();

        let outcomes = machine.mcp(&["install", "--agent", agent]);
        assert_eq!(result(&outcomes), ["added"], "{agent}");
        assert_eq!(
            std::fs::read_link(&link).unwrap(),
            Path::new(points_to),
            "{agent}"
        );
        let installed = std::fs::read_to_string(&target).unwrap();
        assert!(installed.contains("origincloud"), "{agent}: {installed}");
        assert!(
            installed.starts_with(&original[..6]),
            "{agent}: {installed}"
        );
        // The copy is beside the target, and nothing is left beside the link.
        let backup = PathBuf::from(outcomes[0]["backup"].as_str().unwrap());
        assert_eq!(backup.parent(), Some(dotfiles.as_path()), "{agent}");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
        assert_eq!(
            std::fs::read_dir(link.parent().unwrap()).unwrap().count(),
            1
        );

        let outcomes = machine.mcp(&["uninstall", "--agent", agent]);
        assert_eq!(result(&outcomes), ["removed"], "{agent}");
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            original,
            "{agent}"
        );
    }
}

/// A file only its owner can read gives a copy and a result only its owner
/// can read.
#[cfg(unix)]
#[test]
fn the_copy_and_the_result_keep_a_private_files_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    let machine = Machine::new();
    let file = machine.home.join(".claude.json");
    std::fs::write(&file, "{\"numStartups\": 1}\n").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();

    let outcomes = machine.mcp(&["install", "--agent", "claude-code"]);
    assert_eq!(result(&outcomes), ["added"]);
    let backup = PathBuf::from(outcomes[0]["backup"].as_str().unwrap());
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&backup), 0o600);

    let outcomes = machine.mcp(&["uninstall", "--agent", "claude-code"]);
    assert_eq!(result(&outcomes), ["removed"]);
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&backup), 0o600);
}
