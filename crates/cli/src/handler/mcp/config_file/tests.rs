//! Golden-file tests of the edits, one per agent of the table, in
//! `crates/cli/tests/agent-config/`: `{agent}.before.{ext}` is a file
//! the agent could have, with other servers, settings and, where the
//! syntax allows them, comments; `{agent}.installed.{ext}` that file after
//! an install, and `{agent}.new.{ext}` the file an install creates.
//! `UPDATE_GOLDEN=1 cargo test` writes the expected files anew, for review
//! in the diff.

use std::path::PathBuf;

use super::*;
use crate::handler::agents::{Agent, AGENTS};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/agent-config")
}

fn extension(agent: &Agent) -> &'static str {
    match agent.mcp.format.syntax {
        Syntax::Toml => "toml",
        Syntax::Json | Syntax::Jsonc => "json",
    }
}

fn read(agent: &Agent, kind: &str) -> String {
    let path = dir().join(format!("{}.{kind}.{}", agent.id, extension(agent)));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn golden(agent: &Agent, kind: &str, actual: &str) {
    let path = dir().join(format!("{}.{kind}.{}", agent.id, extension(agent)));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(
        actual,
        expected,
        "{} isn't what the edit gives",
        path.display()
    );
}

fn entry() -> Entry {
    Entry {
        name: "origincloud".to_owned(),
        command: "origin".to_owned(),
        args: ["mcp", "serve", "--toolsets", "pvs,location", "--read-only"]
            .map(str::to_owned)
            .to_vec(),
    }
}

#[test]
fn an_install_adds_one_entry_and_keeps_the_rest() {
    for agent in AGENTS {
        let format = agent.mcp.format;
        let before = read(agent, "before");
        assert_eq!(
            find(format, &before, "origincloud").unwrap(),
            None,
            "{}",
            agent.id
        );

        let (installed, change) = install(format, Some(&before), &entry()).unwrap();
        assert_eq!(change, Change::Added, "{}", agent.id);
        golden(agent, "installed", &installed);
        let found = find(format, &installed, "origincloud").unwrap().unwrap();
        assert!(
            found.is(&entry(), format) && found.is_origin(),
            "{}: {found:?}",
            agent.id
        );

        // Twice is once.
        let (again, change) = install(format, Some(&installed), &entry()).unwrap();
        assert_eq!(change, Change::Unchanged, "{}", agent.id);
        assert_eq!(again, installed, "{}", agent.id);

        // Uninstalling gives back the file as it was, byte for byte.
        let (removed, change) = uninstall(format, &installed, "origincloud").unwrap();
        assert_eq!(change, Change::Removed, "{}", agent.id);
        assert_eq!(removed, before, "{}", agent.id);
        let (unchanged, change) = uninstall(format, &removed, "origincloud").unwrap();
        assert_eq!(
            (unchanged.as_str(), change),
            (before.as_str(), Change::Absent)
        );
    }
}

#[test]
fn an_install_creates_the_file() {
    for agent in AGENTS {
        let format = agent.mcp.format;
        let (created, change) = install(format, None, &entry()).unwrap();
        assert_eq!(change, Change::Added, "{}", agent.id);
        golden(agent, "new", &created);
        assert_eq!(snippet(format, &entry()), created, "{}", agent.id);
        // An empty file is a file without servers.
        let (from_empty, _) = install(format, Some(""), &entry()).unwrap();
        assert_eq!(from_empty, created, "{}", agent.id);
    }
}

#[test]
fn a_changed_entry_is_updated_in_place_keeping_its_other_keys() {
    for agent in AGENTS {
        let format = agent.mcp.format;
        let (installed, _) = install(format, Some(&read(agent, "before")), &entry()).unwrap();
        // Someone added a key of their own.
        let (with_env, _) = match format.syntax {
            Syntax::Toml => (
                installed.replace(
                    "[mcp_servers.origincloud]\n",
                    "[mcp_servers.origincloud]\nstartup_timeout_sec = 20\n",
                ),
                (),
            ),
            _ => (
                installed.replacen(
                    "\"command\": \"origin\"",
                    "\"command\": \"origin\",\n      \"timeout\": 20000",
                    1,
                ),
                (),
            ),
        };
        assert_ne!(with_env, installed, "{}", agent.id);
        let narrower = Entry {
            args: vec!["mcp".to_owned(), "serve".to_owned()],
            ..entry()
        };
        let (updated, change) = install(format, Some(&with_env), &narrower).unwrap();
        assert_eq!(change, Change::Updated, "{}", agent.id);
        let found = find(format, &updated, "origincloud").unwrap().unwrap();
        assert!(found.is(&narrower, format), "{}: {found:?}", agent.id);
        assert!(
            updated.contains("20"),
            "{}: the added key is kept: {updated}",
            agent.id
        );
        assert!(!updated.contains("--read-only"), "{}: {updated}", agent.id);
        // Only the arguments differ from the file before the update.
        assert_eq!(
            updated,
            with_env.replace(
                r#"["mcp", "serve", "--toolsets", "pvs,location", "--read-only"]"#,
                r#"["mcp", "serve"]"#
            ),
            "{}",
            agent.id
        );
    }
}

#[test]
fn a_file_that_does_not_parse_is_an_error() {
    for agent in AGENTS {
        let format = agent.mcp.format;
        for broken in ["{ \"mcpServers\": ", "[mcp_servers\n", "not a config"] {
            if format.syntax == Syntax::Toml && broken.starts_with('{') {
                continue;
            }
            assert!(
                install(format, Some(broken), &entry()).is_err(),
                "{}: {broken}",
                agent.id
            );
            assert!(
                uninstall(format, broken, "origincloud").is_err(),
                "{}",
                agent.id
            );
            assert!(find(format, broken, "origincloud").is_err(), "{}", agent.id);
        }
    }
}

#[test]
fn comments_are_refused_where_the_format_has_none() {
    let commented = "// mine\n{ \"mcpServers\": {} }\n";
    for agent in AGENTS {
        let format = agent.mcp.format;
        let result = install(format, Some(commented), &entry());
        match format.syntax {
            Syntax::Json => assert!(result.is_err(), "{}", agent.id),
            Syntax::Jsonc => {
                let (text, _) = result.unwrap();
                assert!(text.starts_with("// mine\n"), "{}: {text}", agent.id);
            }
            Syntax::Toml => assert!(result.is_err(), "{}", agent.id),
        }
    }
}

#[test]
fn servers_that_are_not_a_table_are_an_error() {
    let json = Format {
        syntax: Syntax::Json,
        servers: "mcpServers",
        stdio_type: false,
        all_tools: false,
    };
    assert!(install(json, Some("{\"mcpServers\": []}"), &entry()).is_err());
    assert!(install(json, Some("[]"), &entry()).is_err());
    let toml = Format {
        syntax: Syntax::Toml,
        servers: "mcp_servers",
        stdio_type: false,
        all_tools: false,
    };
    assert!(install(toml, Some("mcp_servers = 1\n"), &entry()).is_err());
}

#[test]
fn only_an_entry_starting_origin_mcp_serve_is_ours() {
    let found = |command: &str, args: &[&str]| Found {
        command: Some(command.to_owned()),
        args: Some(args.iter().map(|a| (*a).to_owned()).collect()),
        kind: None,
        tools: None,
    };
    assert!(found("origin", &["mcp", "serve"]).is_origin());
    assert!(found("/usr/local/bin/origin", &["--config", "/c", "mcp", "serve"]).is_origin());
    assert!(!found("npx", &["origin", "mcp", "serve"]).is_origin());
    assert!(!found("origin", &["serve"]).is_origin());
    assert!(!Found::default().is_origin());
}
