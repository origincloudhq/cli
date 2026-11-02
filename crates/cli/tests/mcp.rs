//! `origin mcp serve` speaks MCP on standard input and output: it answers an
//! MCP client's handshake and lists the tools its flags select, and prints
//! nothing else there.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

/// The messages `origin mcp serve {args}` answers to a client that
/// initializes, lists the tools and closes its input.
fn session(args: &[&str]) -> Vec<Value> {
    let home = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_origin"))
        .args(["mcp", "serve"])
        .args(args)
        .env_clear()
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .env("XDG_DATA_HOME", home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("origin runs");
    let requests = [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "0" },
        }}),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    ];
    let mut stdin = child.stdin.take().unwrap();
    for request in requests {
        writeln!(stdin, "{request}").unwrap();
    }
    // The answers are read before the input closes: closing it ends the
    // session, possibly before the last answer is written.
    let stdout = child.stdout.take().unwrap();
    let mut answers = Vec::new();
    for line in std::io::BufRead::lines(std::io::BufReader::new(stdout)) {
        let line = line.unwrap();
        let answer: Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("standard output holds only MCP messages: {line:?}: {e}"));
        answers.push(answer);
        if answers.iter().any(|a| a["id"] == 2) {
            break;
        }
    }
    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success(), "origin mcp serve exits cleanly: {status}");
    answers
}

fn tools(answers: &[Value]) -> Vec<String> {
    let listed = answers
        .iter()
        .find(|a| a["id"] == 2)
        .expect("the tools are listed");
    listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn it_serves_every_toolset() {
    let answers = session(&[]);
    let initialized = answers.iter().find(|a| a["id"] == 1).unwrap();
    assert_eq!(initialized["result"]["serverInfo"]["name"], "origincloud");
    let tools = tools(&answers);
    for tool in [
        "current_context",
        "pvs_create_volume",
        "pvs_wait_operation",
        "location_list_zones",
    ] {
        assert!(tools.iter().any(|t| t == tool), "{tool} in {tools:?}");
    }
}

#[test]
fn read_only_and_toolsets_narrow_it() {
    let tools = tools(&session(&["--read-only", "--toolsets", "pvs"]));
    assert!(tools.iter().any(|t| t == "pvs_get_volume"), "{tools:?}");
    assert!(!tools.iter().any(|t| t == "pvs_delete_volume"), "{tools:?}");
    assert!(
        !tools.iter().any(|t| t.starts_with("location_")),
        "{tools:?}"
    );
}

#[test]
fn an_unknown_toolset_is_refused() {
    let output = Command::new(env!("CARGO_BIN_EXE_origin"))
        .args(["mcp", "serve", "--toolsets", "no-such-service"])
        .env_clear()
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "nothing on standard output");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no-such-service"), "{stderr}");
}
