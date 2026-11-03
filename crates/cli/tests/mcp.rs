//! `origin mcp serve` speaks MCP on standard input and output: it answers an
//! MCP client's handshake and lists the tools its flags select, and prints
//! nothing else there.

mod common;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

/// The messages `origin mcp serve {args}` answers to a client that
/// initializes, lists the tools and closes its input.
fn session(args: &[&str]) -> Vec<Value> {
    let home = tempfile::tempdir().unwrap();
    exchange(
        args,
        home.path(),
        &[],
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    )
}

/// The messages `origin mcp serve {args}`, with `home` and `env`, answers
/// to a client that initializes, sends `request` (with the id 2) and closes
/// its input.
fn exchange(args: &[&str], home: &Path, env: &[(&str, &str)], request: Value) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_origin"))
        .args(["mcp", "serve"])
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home)
        .env("XDG_DATA_HOME", home)
        .envs(env.iter().copied())
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
        request,
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

/// The region `current_context` tells the agent, with `env` and the
/// profile file `profile` holds.
fn context_region(env: &[(&str, &str)], profile: &str) -> Value {
    let home = tempfile::tempdir().unwrap();
    let file = home.path().join("origincloud").join("default-profile.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, profile).unwrap();
    let answers = exchange(
        &[],
        home.path(),
        env,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
            "name": "current_context", "arguments": {},
        }}),
    );
    let called = answers.iter().find(|a| a["id"] == 2).unwrap();
    let context = &called["result"]["structuredContent"];
    assert!(context["defaults"].is_object(), "{called}");
    context["defaults"]["region"].clone()
}

/// `current_context` tells the agent the region the commands use:
/// `ORIGINCLOUD_REGION`, else the profile's `region`, else the region of
/// the default zone.
#[test]
fn the_context_region_is_the_commands() {
    assert_eq!(context_region(&[], "{}"), Value::Null);
    let zone = r#"{"default_zone":"eu-central1-az1"}"#;
    assert_eq!(context_region(&[], zone), "eu-central1");
    let both = r#"{"default_zone":"eu-central1-az1","region":"eu-central2"}"#;
    assert_eq!(context_region(&[], both), "eu-central2");
    assert_eq!(
        context_region(&[("ORIGINCLOUD_REGION", "eu-central3")], both),
        "eu-central3"
    );
}

/// The tools take whole names as the API does, an instance type's dots and
/// a public image included: they reach the server as given.
#[tokio::test(flavor = "multi_thread")]
async fn the_tools_take_dotted_ids_and_public_images() {
    const LAUNCHING: &str = r#"{"name":"projects/p/zones/z/operations/op-1","done":true,"response":{"@type":"type.googleapis.com/origincloud.compute.v1beta1.Instance","name":"projects/p/zones/z/instances/i"}}"#;
    let server = common::recording(|_| fake_server::Response::json(LAUNCHING)).await;
    let url = server.server.url();
    let home = server.home.path().to_owned();
    let call = |name: &'static str, arguments: Value| {
        let (url, home) = (url.clone(), home.clone());
        tokio::task::spawn_blocking(move || {
            exchange(
                &["--toolsets", "compute"],
                &home,
                &[("ORIGINCLOUD_COMPUTE_ENDPOINT", &url)],
                json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
                    "name": name, "arguments": arguments,
                }}),
            )
        })
    };

    let answers = call(
        "compute_get_instance_type",
        json!({ "name": "instanceTypes/gp1.core.2" }),
    )
    .await
    .unwrap();
    let called = answers.iter().find(|a| a["id"] == 2).unwrap();
    assert_ne!(called["result"]["isError"], true, "{called}");
    let answers = call(
        "compute_create_instance",
        json!({
            "parent": "projects/p/zones/z",
            "instance": {
                "instanceType": "instanceTypes/gp1.core.2",
                "image": "zones/z/images/img-1",
                "keyPair": "projects/p/keyPairs/k",
                "networkInterfaces": ["projects/p/zones/z/networkInterfaces/ni-1"],
            },
        }),
    )
    .await
    .unwrap();
    let called = answers.iter().find(|a| a["id"] == 2).unwrap();
    assert_ne!(called["result"]["isError"], true, "{called}");

    let requests = server.requests();
    assert_eq!(requests[0].path, "/v1beta1/instanceTypes/gp1.core.2");
    assert_eq!(requests[1].path, "/v1beta1/projects/p/zones/z/instances");
    let body: Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(body["instanceType"], "instanceTypes/gp1.core.2");
    assert_eq!(body["image"], "zones/z/images/img-1");
}
