//! The flags of nested and repeated fields, through `origin scratch`, the
//! command group of a service that exists only in the tests
//! (`testing/scratch/`), against a local server that records every
//! request: what each flag sends, how an update names and replaces a whole
//! field, and what the parser refuses before anything is sent.
//!
//! The group is compiled with the `test-services` feature:
//! `cargo test -p origincloud-cli --features test-services`.
#![cfg(feature = "test-services")]

mod common;

use common::{failure, recording, stdout, success, Recording};
use fake_server::{Received, Response};
use serde_json::{json, Value};

/// Answers every call with the firewall the request carried, its name
/// added, as the API answers a create or an update.
fn echo(request: &Received) -> Response {
    let mut firewall: Value = serde_json::from_slice(&request.body).unwrap_or_else(|_| json!({}));
    if let Some(object) = firewall.as_object_mut() {
        object.insert("name".to_owned(), json!("projects/p/zones/z/firewalls/f"));
    }
    Response::json(firewall.to_string())
}

const FIREWALL: [&str; 6] = ["--project", "p", "--zone", "z", "--firewall", "f"];

fn body(request: &Received) -> Value {
    serde_json::from_slice(&request.body).unwrap()
}

async fn scratch(server: &Recording, args: &[&str]) -> std::process::Output {
    server.run("scratch", args).await
}

/// A create states its rules as KEY=VALUE pairs, one flag per rule in the
/// order given, each key typed and its resource names completed like any
/// flag's; repeated scalars and references are repeatable flags; a nested
/// message is one flag. The command prints the API's JSON.
#[tokio::test(flavor = "multi_thread")]
async fn a_create_sends_nested_and_repeated_fields() {
    let server = recording(echo).await;
    let output = scratch(
        &server,
        &[
            "create-firewall",
            "--project",
            "p",
            "--zone",
            "z",
            "--rule",
            "protocol=TCP,port=443,cidr=0.0.0.0/0",
            "--rule",
            "protocol=UDP,network=n-1,idle_timeout=1m30s,allow=true",
            "--port",
            "80",
            "--port",
            "443",
            "--network",
            "n-1",
            "--network",
            "projects/p/zones/z/networks/n-2",
            "--alias",
            "b",
            "--alias",
            "a",
            "--logging",
            "enabled=true,sample_percent=10",
            "--tag",
            "team=web",
        ],
    )
    .await;
    let printed = success(&output);

    let requests = server.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/projects/p/zones/z/firewalls");
    let sent = body(&requests[0]);
    assert_eq!(
        sent,
        json!({
            "rules": [
                {"protocol": "TCP", "port": 443, "cidr": "0.0.0.0/0"},
                {
                    "protocol": "UDP",
                    "network": "projects/p/zones/z/networks/n-1",
                    "idleTimeout": "90s",
                    "allow": true
                }
            ],
            "ports": [80, 443],
            "networks": ["projects/p/zones/z/networks/n-1", "projects/p/zones/z/networks/n-2"],
            "aliases": ["b", "a"],
            "logging": {"enabled": true, "samplePercent": 10},
            "tags": {"team": "web"}
        })
    );
    // The output is the API's JSON, unchanged.
    let mut expected = sent;
    expected["name"] = json!("projects/p/zones/z/firewalls/f");
    assert_eq!(printed, expected);
}

/// Every message field has a JSON flag, the escape hatch for what the
/// KEY=VALUE form can't say: here a rule's `match`, two levels down, and a
/// value with a comma. It takes `@FILE` too.
#[tokio::test(flavor = "multi_thread")]
async fn a_json_flag_sets_a_field_two_levels_deep() {
    let server = recording(echo).await;
    let rules = r#"[{"protocol":"TCP","cidr":"10.0.0.0/8","match":{"headers":[{"name":"x-env","value":"a,b"}]}}]"#;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("logging.json");
    std::fs::write(&file, r#"{"enabled": true}"#).unwrap();
    let logging = format!("@{}", file.display());
    let output = scratch(
        &server,
        &[
            "create-firewall",
            "--project",
            "p",
            "--zone",
            "z",
            "--rules-json",
            rules,
            "--logging-json",
            &logging,
        ],
    )
    .await;
    success(&output);
    let sent = body(&server.requests()[0]);
    assert_eq!(
        sent["rules"],
        json!([{"protocol": "TCP", "cidr": "10.0.0.0/8", "match": {"headers": [{"name": "x-env", "value": "a,b"}]}}])
    );
    assert_eq!(sent["logging"], json!({"enabled": true}));
}

/// A JSON flag refuses a field the message doesn't have, at any depth,
/// naming its path, before anything is sent: the SDK's decoder would drop
/// it, and the rule go without it. A field is named as the command prints
/// it, in camel case, or as the API definitions do.
#[tokio::test(flavor = "multi_thread")]
async fn a_json_flag_refuses_a_field_the_message_doesnt_have() {
    let server = recording(echo).await;
    let create = ["create-firewall", "--project", "p", "--zone", "z"];
    for (flag, value, says) in [
        (
            "--rules-json",
            r#"[{"protocl": "TCP"}]"#,
            "[0].protocl isn't a field of Rule",
        ),
        (
            "--rules-json",
            r#"[{"protocol": "TCP"}, {"protocol": "UDP", "match": {"headrs": []}}]"#,
            "[1].match.headrs isn't a field of Match",
        ),
        (
            "--rules-json",
            r#"[{"protocol": "TCP", "match": {"headers": [{"name": "a"}, {"nam": "b"}]}}]"#,
            "[0].match.headers[1].nam isn't a field of Header",
        ),
        (
            "--logging-json",
            r#"{"enabled": true, "samplePercnt": 5}"#,
            "samplePercnt isn't a field of Logging",
        ),
    ] {
        let mut args = create.to_vec();
        if flag != "--rules-json" {
            args.extend(["--rule", "protocol=TCP"]);
        }
        args.extend([flag, value]);
        let output = scratch(&server, &args).await;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{value}: {stderr}");
        assert!(stderr.contains(flag), "{value}: {stderr}");
        assert!(stderr.contains(says), "{says}: {stderr}");
    }
    assert!(server.requests().is_empty(), "{:?}", server.requests());

    // Camel case, as the command prints it, and the definitions' snake case
    // both name a field, and are sent alike.
    for (rules, logging) in [
        (
            r#"[{"protocol": "TCP", "idleTimeout": "30s"}]"#,
            r#"{"samplePercent": 5}"#,
        ),
        (
            r#"[{"protocol": "TCP", "idle_timeout": "30s"}]"#,
            r#"{"sample_percent": 5}"#,
        ),
    ] {
        let args = [
            &create[..],
            &["--rules-json", rules, "--logging-json", logging],
        ]
        .concat();
        success(&scratch(&server, &args).await);
    }
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    for r in &requests {
        let sent = body(r);
        assert_eq!(
            sent["rules"],
            json!([{"protocol": "TCP", "idleTimeout": "30s"}])
        );
        assert_eq!(sent["logging"], json!({"samplePercent": 5}));
    }
}

/// The parser refuses, before anything is sent, a nested value that isn't
/// one: a required key missing, a value of the wrong type, an unknown key,
/// a key only the JSON flag sets, a key given twice, both forms of one
/// field, JSON that isn't, or no rules at all.
#[tokio::test(flavor = "multi_thread")]
async fn a_bad_nested_value_is_refused_before_any_request() {
    let server = recording(echo).await;
    let create = ["create-firewall", "--project", "p", "--zone", "z"];
    for (args, says) in [
        (&["--rule", "port=443"][..], "protocol is required"),
        (&["--rule", "protocol=ICMP"], "isn't one of TCP, UDP"),
        (
            &["--rule", "protocol=PROTOCOL_UNSPECIFIED"],
            "isn't one of TCP, UDP",
        ),
        (
            &["--rule", "protocol=TCP,port=https"],
            "port: \"https\" isn't a number",
        ),
        (
            &["--rule", "protocol=TCP,colour=red"],
            "colour isn't a key; the keys are protocol, port",
        ),
        (
            &["--rule", "protocol=TCP,match=x"],
            "match is set only through the flag's -json form",
        ),
        (
            &["--rule", "protocol=TCP,port=1,port=2"],
            "port is given twice",
        ),
        (
            &["--rule", "protocol=TCP,idle_timeout=soon"],
            "idle_timeout:",
        ),
        (
            &["--rule", "protocol=TCP,allow=yes"],
            "allow: \"yes\" isn't true or false",
        ),
        (
            &["--rule", "protocol=TCP", "--rules-json", "[]"],
            "cannot be used with",
        ),
        (&["--rules-json", "[{"], "not JSON"),
        (
            &["--rule", "protocol=TCP", "--logging", "enabled=yes"],
            "enabled",
        ),
        (&["--port", "80"], "--rule"),
    ] {
        let output = scratch(&server, &[&create[..], args].concat()).await;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains(says), "{args:?}: {stderr}");
    }
    // A resource name a key gives follows the id rules of every flag.
    for network in ["..", "a/b", "projects/p/zones/../networks/n"] {
        let rule = format!("protocol=TCP,network={network}");
        let description =
            failure(&scratch(&server, &[&create[..], &["--rule", &rule]].concat()).await);
        assert!(
            description.starts_with("--rule network:"),
            "{network}: {description}"
        );
    }
    assert!(server.requests().is_empty(), "{:?}", server.requests());
}

/// An update names in its mask the top-level fields its flags set, and
/// sends each whole: a nested message's keys not given are reset, a list
/// is replaced by the elements given, and `--no-…` empties a field by
/// name.
#[tokio::test(flavor = "multi_thread")]
async fn an_update_replaces_whole_fields() {
    let server = recording(echo).await;
    let update =
        |extra: &'static [&'static str]| [&["update-firewall"][..], &FIREWALL, extra].concat();
    for args in [
        update(&["--rule", "protocol=TCP,port=22", "--rule", "protocol=UDP"]),
        update(&["--logging", "sample_percent=5"]),
        update(&["--no-rules", "--no-ports", "--no-logging"]),
        update(&["--rules-json", "[]", "--alias", "x"]),
        update(&["--network", "n-3"]),
    ] {
        success(&scratch(&server, &args).await);
    }
    let requests = server.requests();
    assert_eq!(requests.len(), 5, "{requests:?}");
    for r in &requests {
        assert_eq!(r.method, "PATCH");
        assert_eq!(r.path, "/v1/projects/p/zones/z/firewalls/f");
    }
    let mask = |r: &Received| -> Vec<String> {
        let mut paths: Vec<String> = r
            .query_values("updateMask")
            .iter()
            .flat_map(|v| v.split(',').map(str::to_owned).collect::<Vec<_>>())
            .collect();
        paths.sort();
        paths
    };
    assert_eq!(mask(&requests[0]), ["rules"]);
    assert_eq!(
        body(&requests[0])["rules"],
        json!([{"protocol": "TCP", "port": 22}, {"protocol": "UDP"}])
    );
    assert_eq!(mask(&requests[1]), ["logging"]);
    assert_eq!(body(&requests[1])["logging"], json!({"samplePercent": 5}));
    assert_eq!(mask(&requests[2]), ["logging", "ports", "rules"]);
    let emptied = body(&requests[2]);
    for field in ["rules", "ports", "logging"] {
        assert!(emptied.get(field).is_none(), "{field}: {emptied}");
    }
    assert_eq!(mask(&requests[3]), ["aliases", "rules"]);
    assert_eq!(body(&requests[3])["aliases"], json!(["x"]));
    assert_eq!(mask(&requests[4]), ["networks"]);
    assert_eq!(
        body(&requests[4])["networks"],
        json!(["projects/p/zones/z/networks/n-3"])
    );

    // Setting and emptying one field, both forms of one field, or nothing
    // at all: refused by the parser, nothing sent.
    for args in [
        update(&["--port", "1", "--no-ports"]),
        update(&["--rule", "protocol=TCP", "--no-rules"]),
        update(&["--rules-json", "[]", "--no-rules"]),
        update(&["--logging", "enabled=true", "--logging-json", "{}"]),
        update(&[]),
    ] {
        let output = scratch(&server, &args).await;
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stdout(&output)
        );
    }
    assert_eq!(server.requests().len(), 5);
}

/// The help lists a nested flag's keys, says what only the JSON flag sets,
/// and warns that an update replaces a list whole.
#[test]
fn the_help_lists_the_keys() {
    let root = origincloud_cli::command();
    let group = root.find_subcommand("scratch").unwrap();
    let help = |command: &str, flag: &str| -> String {
        let arg = group
            .find_subcommand(command)
            .unwrap()
            .get_arguments()
            .find(|a| a.get_long() == Some(flag))
            .unwrap_or_else(|| panic!("{command} has no --{flag}"));
        arg.get_long_help()
            .or(arg.get_help())
            .map(ToString::to_string)
            .unwrap_or_default()
    };
    let rule = help("create-firewall", "rule");
    for says in [
        "`protocol`, required:",
        "One of `TCP`, `UDP`",
        "`idle_timeout`:",
        "`match` is set only through `--rules-json`",
    ] {
        assert!(rule.contains(says), "{says}: {rule}");
    }
    assert!(help("update-firewall", "rule").contains("The list is replaced whole"));
    assert!(help("update-firewall", "logging").contains("a key not given is reset"));
    assert!(help("create-firewall", "alias").contains("doesn't keep their order"));
    assert!(help("create-firewall", "rules-json").contains("`@FILE`"));
}
