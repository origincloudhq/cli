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
            "--notify-email",
            "ops@example.com",
            "--project",
            "p",
            "--zone",
            "z",
            "--rule",
            "protocol=TCP,port=443,cidr=0.0.0.0/0",
            "--rule",
            "protocol=UDP,network=//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-1,idle_timeout=1m30s,allow=true",
            "--port",
            "80",
            "--port",
            "443",
            "--network",
            "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-1",
            "--network",
            "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-2",
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
                    "network": "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-1",
                    "idleTimeout": "90s",
                    "allow": true
                }
            ],
            "ports": [80, 443],
            "networks": ["//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-1", "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-2"],
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
            "--notify-email",
            "ops@example.com",
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
    let create = [
        "create-firewall",
        "--project",
        "p",
        "--zone",
        "z",
        "--notify-email",
        "ops@example.com",
    ];
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
    let create = [
        "create-firewall",
        "--project",
        "p",
        "--zone",
        "z",
        "--notify-email",
        "ops@example.com",
    ];
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
            "match is set only through --rules-json",
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
    for network in [
        "..",
        "a/b",
        "//scratch.origincloudapis.eu/projects/p/zones/../networks/n",
    ] {
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
        update(&["--no-metadata", "--no-ports", "--no-logging"]),
        update(&["--rules-json", "[]", "--alias", "x"]),
        update(&[
            "--network",
            "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-3",
        ]),
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
    assert_eq!(mask(&requests[2]), ["logging", "metadata", "ports"]);
    let emptied = body(&requests[2]);
    for field in ["metadata", "ports", "logging"] {
        assert!(emptied.get(field).is_none(), "{field}: {emptied}");
    }
    assert_eq!(mask(&requests[3]), ["aliases", "rules"]);
    assert_eq!(body(&requests[3])["aliases"], json!(["x"]));
    assert_eq!(mask(&requests[4]), ["networks"]);
    assert_eq!(
        body(&requests[4])["networks"],
        json!(["//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-3"])
    );

    // Setting and emptying one field, both forms of one field, emptying a
    // required one, or nothing at all: refused by the parser, nothing sent.
    for args in [
        update(&["--port", "1", "--no-ports"]),
        update(&["--logging", "enabled=true", "--no-logging"]),
        update(&["--logging-json", "{}", "--no-logging"]),
        update(&["--logging", "enabled=true", "--logging-json", "{}"]),
        update(&["--rule", "protocol=TCP", "--rules-json", "[]"]),
        update(&["--no-rules"]),
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

/// A refusal of the parser: exit status 2, its message on standard error,
/// and nothing sent.
async fn refused(server: &Recording, args: &[&str], says: &str) {
    let output = scratch(server, args).await;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{args:?}: {stderr}");
    assert!(stderr.contains(says), "{says}: {stderr}");
}

/// A create without the choice of whom to notify, which `CREATE` makes.
const BASE: [&str; 5] = ["create-firewall", "--project", "p", "--zone", "z"];
const CREATE: [&str; 7] = [
    "create-firewall",
    "--project",
    "p",
    "--zone",
    "z",
    "--notify-email",
    "ops@example.com",
];

/// A JSON value's enums are checked as their KEY=VALUE form checks them,
/// in a field, a list and a map: a name of the enum's, or its number, never
/// the unspecified value. Numbers, booleans and strings are checked too,
/// and a key given twice; each refusal names its path.
#[tokio::test(flavor = "multi_thread")]
async fn a_json_value_is_checked_value_by_value() {
    let server = recording(echo).await;
    let rules = |json: &'static str| [&CREATE[..], &["--rules-json", json]].concat();
    for (json, says) in [
        (
            r#"[{"protocol": "tcp"}]"#,
            r#"[0].protocol: "tcp" isn't one of TCP, UDP"#,
        ),
        (
            r#"[{"protocol": "BOGUS"}]"#,
            r#"[0].protocol: "BOGUS" isn't one of TCP, UDP"#,
        ),
        (
            r#"[{"protocol": 7}]"#,
            "[0].protocol: 7 isn't one of TCP, UDP",
        ),
        (
            r#"[{"protocol": "PROTOCOL_UNSPECIFIED"}]"#,
            "[0].protocol: \"PROTOCOL_UNSPECIFIED\" isn't one of",
        ),
        (r#"[{"protocol": 0}]"#, "[0].protocol: 0 isn't one of"),
        (
            r#"[{"protocol": "TCP", "match": {"protocols": ["UDP", "icmp"]}}]"#,
            r#"[0].match.protocols[1]: "icmp" isn't one of TCP, UDP"#,
        ),
        (
            r#"[{"protocol": "TCP", "match": {"portProtocols": {"dns": "BOGUS"}}}]"#,
            r#"[0].match.portProtocols["dns"]: "BOGUS" isn't one of TCP, UDP"#,
        ),
        (
            r#"[{"protocol": "TCP", "port": "https"}]"#,
            r#"[0].port: "https" isn't a whole number"#,
        ),
        (
            r#"[{"protocol": "TCP", "allow": "yes"}]"#,
            r#"[0].allow: "yes" isn't true or false"#,
        ),
        (
            r#"[{"protocol": "TCP", "cidr": 10}]"#,
            "[0].cidr: 10 isn't a string",
        ),
        (
            r#"[{"protocol": "TCP", "idleTimeout": "30"}]"#,
            r#"[0].idleTimeout: "30" isn't a duration"#,
        ),
        (
            r#"[{"protocol": "TCP", "idleTimeout": 30}]"#,
            "[0].idleTimeout: 30 isn't a duration",
        ),
        (
            r#"[{"protocol": "TCP", "notAfter": "2026-11-01"}]"#,
            r#"[0].notAfter: "2026-11-01" isn't an RFC 3339 time"#,
        ),
        (
            r#"[{"protocol": "TCP", "protocol": "UDP"}]"#,
            "[0].protocol is given twice",
        ),
        (
            r#"[{"protocol": "TCP", "idle_timeout": "1s", "idleTimeout": "2s"}]"#,
            "[0].idle_timeout is [0].idleTimeout again",
        ),
    ] {
        refused(&server, &rules(json), says).await;
    }
    assert!(server.requests().is_empty(), "{:?}", server.requests());

    // A number names an enum's value as its name does.
    success(&scratch(&server, &rules(r#"[{"protocol": 1}]"#)).await);
    assert_eq!(
        body(&server.requests()[0])["rules"],
        json!([{"protocol": "TCP"}])
    );
}

/// A resource name in a nested value, a key or JSON at any depth, is a
/// full name, of another project too; a relative name or an id is refused,
/// as it is in a flag, before anything is sent.
#[tokio::test(flavor = "multi_thread")]
async fn resource_names_in_nested_values_are_full_names() {
    let server = recording(echo).await;
    let rules = r#"[{"protocol": "TCP", "network": "//scratch.origincloudapis.eu/projects/q/zones/z/networks/n-1", "match": {"network": "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-2"}}]"#;
    success(&scratch(&server, &[&CREATE[..], &["--rules-json", rules]].concat()).await);
    success(
        &scratch(
            &server,
            &[
                &CREATE[..],
                &[
                    "--rule",
                    "protocol=TCP,network=//scratch.origincloudapis.eu/projects/q/zones/z/networks/n-1",
                ],
            ]
            .concat(),
        )
        .await,
    );
    assert_eq!(
        body(&server.requests()[0])["rules"],
        json!([{
            "protocol": "TCP",
            "network": "//scratch.origincloudapis.eu/projects/q/zones/z/networks/n-1",
            "match": {"network": "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-2"}
        }])
    );
    assert_eq!(
        body(&server.requests()[1])["rules"],
        json!([{"protocol": "TCP", "network": "//scratch.origincloudapis.eu/projects/q/zones/z/networks/n-1"}])
    );

    for (rules, says) in [
        (
            r#"[{"protocol": "TCP", "network": "n-1"}]"#,
            r#"[0].network: "n-1" is an id; give the resource's full name"#,
        ),
        (
            r#"[{"protocol": "TCP", "match": {"network": "n-2"}}]"#,
            r#"[0].match.network: "n-2" is an id; give the resource's full name"#,
        ),
        (
            r#"[{"protocol": "TCP", "match": {"network": "projects/p/zones/z/networks/n-2"}}]"#,
            r#"[0].match.network: "projects/p/zones/z/networks/n-2" is a relative name; give the resource's full name"#,
        ),
    ] {
        refused(
            &server,
            &[&CREATE[..], &["--rules-json", rules]].concat(),
            says,
        )
        .await;
    }
    let description = failure(
        &scratch(
            &server,
            &[&CREATE[..], &["--rule", "protocol=TCP,network=n-1"]].concat(),
        )
        .await,
    );
    assert!(
        description.starts_with(
            "--rule network: \"n-1\" is an id; give the resource's full name, such as \
             //scratch.origincloudapis.eu/projects/{project}/zones/{zone}/networks/{network}"
        ),
        "{description}"
    );
    let description = failure(
        &scratch(
            &server,
            &[
                &CREATE[..],
                &[
                    "--rule",
                    "protocol=TCP,network=projects/q/zones/z/networks/n-1",
                ],
            ]
            .concat(),
        )
        .await,
    );
    assert!(
        description
            .starts_with("--rule network: \"projects/q/zones/z/networks/n-1\" is a relative name"),
        "{description}"
    );
    assert_eq!(server.requests().len(), 2);
}

/// A reference whose resource has several name patterns, as the scratch
/// network has, takes a full name of each wherever it is given: a
/// repeatable flag, a oneof's member, a key and a JSON value, each sent as
/// its full name, whatever `--zone` says. An id may hold dots in a full
/// name; a relative name or an id alone is refused.
#[tokio::test(flavor = "multi_thread")]
async fn a_reference_takes_a_full_name_of_each_pattern_and_dotted_ids() {
    let server = recording(echo).await;
    success(
        &scratch(
            &server,
            &[
                &CREATE[..],
                &[
                    "--rule",
                    "protocol=TCP,network=//scratch.origincloudapis.eu/zones/z/networks/shared.2",
                    "--rule",
                    "protocol=UDP,network=//scratch.origincloudapis.eu/projects/p/zones/z/networks/n.3",
                    "--network",
                    "//scratch.origincloudapis.eu/zones/z2/networks/shared-1",
                    "--network",
                    "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n.2",
                    "--mirror-network",
                    "//scratch.origincloudapis.eu/zones/z/networks/m.1",
                ],
            ]
            .concat(),
        )
        .await,
    );
    let sent = body(&server.requests()[0]);
    assert_eq!(
        sent["networks"],
        json!([
            "//scratch.origincloudapis.eu/zones/z2/networks/shared-1",
            "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n.2"
        ])
    );
    assert_eq!(
        sent["rules"],
        json!([
            {"protocol": "TCP", "network": "//scratch.origincloudapis.eu/zones/z/networks/shared.2"},
            {"protocol": "UDP", "network": "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n.3"}
        ])
    );
    assert_eq!(
        sent["mirrorNetwork"],
        "//scratch.origincloudapis.eu/zones/z/networks/m.1"
    );

    let rules =
        r#"[{"protocol": "TCP", "network": "//scratch.origincloudapis.eu/zones/z/networks/s.1"}]"#;
    success(&scratch(&server, &[&CREATE[..], &["--rules-json", rules]].concat()).await);
    assert_eq!(
        body(&server.requests()[1])["rules"],
        json!([{"protocol": "TCP", "network": "//scratch.origincloudapis.eu/zones/z/networks/s.1"}])
    );

    // An update, whose firewall's name is learned from a whole name.
    success(
        &scratch(
            &server,
            &[
                "update-firewall",
                "--firewall",
                "projects/p/zones/z/firewalls/f.1",
                "--network",
                "//scratch.origincloudapis.eu/zones/z/networks/shared-1",
            ],
        )
        .await,
    );
    let update = &server.requests()[2];
    assert_eq!(update.path, "/v1/projects/p/zones/z/firewalls/f.1");
    assert_eq!(
        body(update)["networks"],
        json!(["//scratch.origincloudapis.eu/zones/z/networks/shared-1"])
    );

    // A relative name, an id, a name of neither form, or one with a part
    // that could leave its place, is refused.
    for (args, says) in [
        (&["--network", "n.1"][..], "--network: \"n.1\" is an id"),
        (
            &["--network", "zones/z/networks/shared-1"],
            "--network: \"zones/z/networks/shared-1\" is a relative name; give the resource's \
             full name, such as \
             //scratch.origincloudapis.eu/projects/{project}/zones/{zone}/networks/{network}",
        ),
        (
            &["--mirror-network", "projects/p/zones/z/networks/n-1"],
            "--mirror-network: \"projects/p/zones/z/networks/n-1\" is a relative name",
        ),
        (
            &["--mirror-network", "m-1"],
            "--mirror-network: \"m-1\" is an id",
        ),
        (
            &["--network", "//scratch.origincloudapis.eu/networks/s"],
            "isn't a name of the form projects/{project}/zones/{zone}/networks/{network} or \
             zones/{zone}/networks/{network}",
        ),
        (
            &[
                "--network",
                "//scratch.origincloudapis.eu/zones/z/networks/..",
            ],
            "isn't a name of the form",
        ),
        (&["--network", ".."], "is an id"),
    ] {
        let description = failure(
            &scratch(
                &server,
                &[&CREATE[..], args, &["--rule", "protocol=TCP"]].concat(),
            )
            .await,
        );
        assert!(description.contains(says), "{args:?}: {description}");
    }
    assert_eq!(server.requests().len(), 3);

    // The help names both full forms, and no relative one.
    let help = origincloud_cli::command()
        .find_subcommand("scratch")
        .and_then(|g| g.find_subcommand("create-firewall"))
        .unwrap()
        .get_arguments()
        .find(|a| a.get_long() == Some("network"))
        .and_then(|a| a.get_long_help().or(a.get_help()))
        .map(|h| h.to_string())
        .unwrap();
    for form in [
        "full name, `//scratch.origincloudapis.eu/projects/{project}/zones/{zone}/networks/{network}` \
         or `//scratch.origincloudapis.eu/zones/{zone}/networks/{network}`",
        "not a relative name or an id",
    ] {
        assert!(help.contains(form), "{form}: {help}");
    }
    assert!(!help.contains("its relative name"), "{help}");
}

/// A key is a field's name with `_` or `-`; a timestamp, as a flag or a
/// key, is RFC 3339; a list the API keeps unordered has each element once,
/// in a flag or in JSON.
#[tokio::test(flavor = "multi_thread")]
async fn kebab_keys_timestamps_and_unordered_lists() {
    let server = recording(echo).await;
    success(
        &scratch(
            &server,
            &[
                &CREATE[..],
                &[
                    "--rule",
                    "protocol=TCP,idle-timeout=30s,not-after=2026-11-01T12:00:00+01:00",
                    "--expire-time",
                    "2026-12-24T18:00:00Z",
                ],
            ]
            .concat(),
        )
        .await,
    );
    let sent = body(&server.requests()[0]);
    assert_eq!(
        sent["rules"],
        json!([{"protocol": "TCP", "idleTimeout": "30s", "notAfter": "2026-11-01T11:00:00Z"}])
    );
    assert_eq!(sent["expireTime"], "2026-12-24T18:00:00Z");

    refused(
        &server,
        &[
            &CREATE[..],
            &["--rule", "protocol=TCP", "--expire-time", "tomorrow"],
        ]
        .concat(),
        "isn't an RFC 3339 time",
    )
    .await;
    refused(
        &server,
        &[
            &CREATE[..],
            &["--rule", "protocol=TCP,not_after=2026-11-01"],
        ]
        .concat(),
        "not_after:",
    )
    .await;
    refused(
        &server,
        &[
            &CREATE[..],
            &[
                "--rules-json",
                r#"[{"protocol": "TCP", "match": {"methods": ["GET", "PUT", "GET"]}}]"#,
            ],
        ]
        .concat(),
        "[0].match.methods[2] is [0].match.methods[0] again",
    )
    .await;
    let description = failure(
        &scratch(
            &server,
            &[
                &CREATE[..],
                &["--rule", "protocol=TCP", "--alias", "a", "--alias", "a"],
            ]
            .concat(),
        )
        .await,
    );
    assert_eq!(
        description,
        "--alias: the 2nd value is the 1st again; the API keeps each once"
    );
    assert_eq!(server.requests().len(), 1);
}

/// An update says on standard error what it replaces whole: the keys of a
/// nested message it resets, and that a list's elements are all replaced.
#[tokio::test(flavor = "multi_thread")]
async fn an_update_says_what_it_replaces() {
    let server = recording(echo).await;
    let update = |extra: &[&'static str]| [&["update-firewall"][..], &FIREWALL, extra].concat();
    for (args, says) in [
        (
            update(&["--logging", "sample_percent=5"]),
            "--logging leaves out enabled: the update replaces logging whole, so it resets them.",
        ),
        (
            update(&["--logging-json", r#"{"enabled": true}"#]),
            "--logging-json leaves out sample_percent: the update replaces logging whole, so it resets them.",
        ),
        (
            update(&["--rule", "protocol=TCP", "--rule", "protocol=UDP"]),
            "--rule: the update replaces all of rules with the 2 given.",
        ),
        (
            update(&["--rules-json", r#"[{"protocol": "TCP"}]"#]),
            "--rules-json: the update replaces all of rules with the 1 given.",
        ),
    ] {
        let output = scratch(&server, &args).await;
        success(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(says), "{says}: {stderr}");
    }
    // Every key given: nothing to say.
    let output = scratch(
        &server,
        &update(&["--logging", "enabled=true,sample-percent=5"]),
    )
    .await;
    success(&output);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("leaves out"));
}

/// A well-known type's JSON flag takes its own JSON form: a struct is any
/// object, and its help says nothing of fields or keys.
#[tokio::test(flavor = "multi_thread")]
async fn a_well_known_type_takes_its_own_json() {
    let server = recording(echo).await;
    success(
        &scratch(
            &server,
            &[
                &CREATE[..],
                &[
                    "--rule",
                    "protocol=TCP",
                    "--metadata-json",
                    r#"{"owner": {"team": "web"}, "n": 1}"#,
                ],
            ]
            .concat(),
        )
        .await,
    );
    assert_eq!(
        body(&server.requests()[0])["metadata"],
        // A struct's number is a double, as protobuf has it.
        json!({"owner": {"team": "web"}, "n": 1.0})
    );
}

/// Every message flag's one-line help says it sends, or in an update
/// replaces, the field whole; a required field has no `--no-` flag.
#[test]
fn the_one_line_help_says_a_field_is_replaced_whole() {
    let root = origincloud_cli::command();
    let group = root.find_subcommand("scratch").unwrap();
    let short = |command: &str, flag: &str| -> String {
        group
            .find_subcommand(command)
            .unwrap()
            .get_arguments()
            .find(|a| a.get_long() == Some(flag))
            .unwrap_or_else(|| panic!("{command} has no --{flag}"))
            .get_help()
            .map(ToString::to_string)
            .unwrap_or_default()
    };
    for flag in ["rule", "rules-json", "logging", "logging-json"] {
        assert!(
            short("update-firewall", flag).contains("replaced whole"),
            "--{flag}: {}",
            short("update-firewall", flag)
        );
        assert!(
            short("create-firewall", flag).contains("sent whole"),
            "--{flag}"
        );
    }
    let metadata = short("update-firewall", "metadata-json");
    assert_eq!(metadata, "Metadata as its JSON value");
    let long = group
        .find_subcommand("update-firewall")
        .unwrap()
        .get_arguments()
        .find(|a| a.get_long() == Some("metadata-json"))
        .unwrap()
        .get_long_help()
        .unwrap()
        .to_string();
    assert!(
        !long.contains("camel case") && !long.contains("reset"),
        "{long}"
    );
    let update = group.find_subcommand("update-firewall").unwrap();
    assert!(update
        .get_arguments()
        .all(|a| a.get_long() != Some("no-rules")));
    assert!(update
        .get_arguments()
        .any(|a| a.get_long() == Some("no-logging")));
}

/// A field with explicit presence (proto3 `optional`) is sent when given,
/// zero or empty included, and left out when not; an update sets it or,
/// with `--no-…`, unsets it, naming it in the mask either way. A key of a
/// nested message works the same.
#[tokio::test(flavor = "multi_thread")]
async fn an_optional_field_is_set_or_unset() {
    let server = recording(echo).await;
    success(
        &scratch(
            &server,
            &[
                &CREATE[..],
                &[
                    "--rule",
                    "protocol=TCP,priority=0",
                    "--rule",
                    "protocol=UDP",
                    "--max-connections",
                    "0",
                    "--strict",
                    "false",
                    "--default-protocol",
                    "UDP",
                    "--note",
                    "",
                ],
            ]
            .concat(),
        )
        .await,
    );
    success(
        &scratch(
            &server,
            &[&CREATE[..], &["--rule", "protocol=TCP"]].concat(),
        )
        .await,
    );
    let given = body(&server.requests()[0]);
    assert_eq!(given["maxConnections"], 0);
    assert_eq!(given["strict"], false);
    assert_eq!(given["defaultProtocol"], "UDP");
    assert_eq!(given["note"], "");
    assert_eq!(
        given["rules"],
        json!([{"protocol": "TCP", "priority": 0}, {"protocol": "UDP"}])
    );
    let left_out = body(&server.requests()[1]);
    for field in ["maxConnections", "strict", "defaultProtocol", "note"] {
        assert!(left_out.get(field).is_none(), "{field}: {left_out}");
    }

    let update = |extra: &[&'static str]| [&["update-firewall"][..], &FIREWALL, extra].concat();
    success(&scratch(&server, &update(&["--max-connections", "0"])).await);
    success(&scratch(&server, &update(&["--no-max-connections", "--no-strict"])).await);
    let requests = server.requests();
    let mask = |r: &Received| {
        let mut paths: Vec<String> = r
            .query_values("updateMask")
            .iter()
            .flat_map(|v| v.split(',').map(str::to_owned).collect::<Vec<_>>())
            .collect();
        paths.sort();
        paths
    };
    assert_eq!(mask(&requests[2]), ["maxConnections"]);
    assert_eq!(body(&requests[2])["maxConnections"], 0);
    assert_eq!(mask(&requests[3]), ["maxConnections", "strict"]);
    let unset = body(&requests[3]);
    assert!(
        unset.get("maxConnections").is_none() && unset.get("strict").is_none(),
        "{unset}"
    );

    refused(
        &server,
        &update(&["--max-connections", "5", "--no-max-connections"]),
        "cannot be used with",
    )
    .await;
    assert_eq!(server.requests().len(), 4);
}

/// A oneof's members are one flag each, of which at most one is given, or
/// exactly one when they are all required; the request carries the one
/// given. An update names every member in the mask, so that the one it sets
/// replaces whichever was set, and `--no-{oneof}` sets none. A member that
/// is a message has both forms of a message field.
#[tokio::test(flavor = "multi_thread")]
async fn a_oneof_is_one_flag_of_its_members() {
    let server = recording(echo).await;
    for extra in [
        &[
            "--mirror-network",
            "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-1",
            "--notify-email",
            "ops@example.com",
        ][..],
        &[
            "--mirror-address",
            "10.0.0.9:4789",
            "--notify-email",
            "ops@example.com",
        ],
        &[
            "--mirror-log",
            "enabled=true",
            "--notify-after-seconds",
            "0",
        ],
        &[
            "--mirror-log-json",
            r#"{"samplePercent": 5}"#,
            "--notify-after-seconds",
            "30",
        ],
    ] {
        let args = [&BASE[..], &["--rule", "protocol=TCP"], extra].concat();
        success(&scratch(&server, &args).await);
    }
    let requests = server.requests();
    assert_eq!(
        body(&requests[0])["mirrorNetwork"],
        "//scratch.origincloudapis.eu/projects/p/zones/z/networks/n-1"
    );
    assert_eq!(body(&requests[1])["mirrorAddress"], "10.0.0.9:4789");
    assert_eq!(requests[1].query_values("notifyEmail"), ["ops@example.com"]);
    assert_eq!(body(&requests[2])["mirrorLog"], json!({"enabled": true}));
    assert_eq!(requests[2].query_values("notifyAfterSeconds"), ["0"]);
    assert_eq!(body(&requests[3])["mirrorLog"], json!({"samplePercent": 5}));
    assert_eq!(requests[3].query_values("notifyAfterSeconds"), ["30"]);
    for (i, r) in requests.iter().enumerate() {
        let sent = body(r);
        let members = ["mirrorNetwork", "mirrorAddress", "mirrorLog"]
            .iter()
            .filter(|m| sent.get(**m).is_some())
            .count();
        assert_eq!(members, 1, "{i}: {sent}");
    }

    // An update: every member in the mask, the one given in the body; a
    // message member says which keys it resets; `--no-mirror` empties it.
    let update = |extra: &[&'static str]| [&["update-firewall"][..], &FIREWALL, extra].concat();
    let every = ["mirrorAddress", "mirrorLog", "mirrorNetwork"];
    let mask = |r: &Received| {
        let mut paths: Vec<String> = r
            .query_values("updateMask")
            .iter()
            .flat_map(|v| v.split(',').map(str::to_owned).collect::<Vec<_>>())
            .collect();
        paths.sort();
        paths
    };
    success(&scratch(&server, &update(&["--mirror-address", "10.0.0.9:4789"])).await);
    let output = scratch(&server, &update(&["--mirror-log", "enabled=true"])).await;
    success(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "--mirror-log leaves out sample_percent: the update replaces mirror_log whole, so it resets them."
        ),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    success(&scratch(&server, &update(&["--no-mirror"])).await);
    let requests = server.requests();
    for r in &requests[4..7] {
        assert_eq!(mask(r), every);
    }
    assert_eq!(
        body(&requests[4]),
        json!({"name": "projects/p/zones/z/firewalls/f", "mirrorAddress": "10.0.0.9:4789"})
    );
    assert_eq!(
        body(&requests[5]),
        json!({"name": "projects/p/zones/z/firewalls/f", "mirrorLog": {"enabled": true}})
    );
    assert_eq!(
        body(&requests[6]),
        json!({"name": "projects/p/zones/z/firewalls/f"})
    );

    for (args, says) in [
        (
            [&CREATE[..], &["--rule", "protocol=TCP", "--mirror-address", "a", "--mirror-network", "n"]].concat(),
            "cannot be used with",
        ),
        (
            [&CREATE[..], &["--rule", "protocol=TCP", "--mirror-address", "a", "--mirror-log-json", "{}"]].concat(),
            "cannot be used with",
        ),
        (
            [&BASE[..], &["--rule", "protocol=TCP", "--notify-email", "a", "--notify-after-seconds", "1"]].concat(),
            "cannot be used with",
        ),
        // A choice of exactly one: a create must make it.
        (
            [&BASE[..], &["--rule", "protocol=TCP"]].concat(),
            "the following required arguments were not provided",
        ),
        (update(&["--mirror-address", "a", "--mirror-log", "enabled=true"]), "cannot be used with"),
        (update(&["--mirror-address", "a", "--no-mirror"]), "cannot be used with"),
        (
            [
                &CREATE[..],
                &[
                    "--rules-json",
                    r#"[{"protocol": "TCP", "match": {"sourceCidr": "10.0.0.0/8", "sourceTag": "web"}}]"#,
                ],
            ]
            .concat(),
            "[0].match.sourceTag and [0].match.sourceCidr are both of Match's choice source: give one",
        ),
    ] {
        refused(&server, &args, says).await;
    }
    assert_eq!(server.requests().len(), 7);

    // A member given as `null` is no member: it is left out.
    success(
        &scratch(
            &server,
            &[
                &CREATE[..],
                &[
                    "--rules-json",
                    r#"[{"protocol": "TCP", "match": {"sourceCidr": "10.0.0.0/8", "sourceTag": null}}]"#,
                ],
            ]
            .concat(),
        )
        .await,
    );
    assert_eq!(
        body(&server.requests()[7])["rules"],
        json!([{"protocol": "TCP", "match": {"sourceCidr": "10.0.0.0/8"}}])
    );
}

/// The help of a oneof's flags and an optional field's `--no-` flag.
#[test]
fn the_help_of_choices_and_optional_fields() {
    let root = origincloud_cli::command();
    let group = root.find_subcommand("scratch").unwrap();
    let arg = |command: &str, flag: &str| {
        group
            .find_subcommand(command)
            .unwrap()
            .get_arguments()
            .find(|a| a.get_long() == Some(flag))
            .unwrap_or_else(|| panic!("{command} has no --{flag}"))
            .clone()
    };
    let help = |a: &clap::Arg| a.get_help().map(ToString::to_string).unwrap_or_default();
    let long = |a: &clap::Arg| {
        a.get_long_help()
            .map(ToString::to_string)
            .unwrap_or_default()
    };
    assert_eq!(
        help(&arg("update-firewall", "no-strict")),
        "Unsets `--strict`; the API's default applies"
    );
    assert!(help(&arg("update-firewall", "mirror-log")).contains("replaced whole"));
    assert!(long(&arg("create-firewall", "notify-email")).contains("give exactly one"));
    assert!(long(&arg("create-firewall", "mirror-address")).contains("give at most one"));
    assert!(long(&arg("update-firewall", "mirror-address")).contains("`--no-mirror` sets none"));
}

/// A list of a top-level collection, with no parent to set, pages through
/// its request until the last page, and prints every page's items as one.
#[tokio::test(flavor = "multi_thread")]
async fn a_top_level_list_pages_through_every_page() {
    let server = recording(|request| {
        Response::json(if request.query.contains("pageToken=next") {
            r#"{"firewallPolicies":[{"name":"firewallPolicies/b"}]}"#
        } else {
            r#"{"firewallPolicies":[{"name":"firewallPolicies/a"}],"nextPageToken":"next"}"#
        })
    })
    .await;
    let printed = success(&scratch(&server, &["list-firewall-policies"]).await);
    assert_eq!(
        printed["firewallPolicies"],
        json!([{"name": "firewallPolicies/a"}, {"name": "firewallPolicies/b"}]),
        "{printed}"
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests
        .iter()
        .all(|r| r.method == "GET" && r.path == "/v1/firewallPolicies"));
}

/// A list filtering on a field rather than a parent takes it as a flag, a
/// whole resource name sent as its full name; a reference to a type no
/// package declares is sent whole, as given.
#[tokio::test(flavor = "multi_thread")]
async fn a_filtered_list_sends_its_filters() {
    let server =
        recording(|_| Response::json(r#"{"networks":[{"name":"projects/p/zones/z/networks/n"}]}"#))
            .await;
    let printed = success(
        &scratch(
            &server,
            &[
                "list-networks",
                "--firewall",
                "//scratch.origincloudapis.eu/projects/p/zones/z/firewalls/f",
                "--owner",
                "people/ada",
            ],
        )
        .await,
    );
    assert_eq!(
        printed["networks"][0]["name"],
        "projects/p/zones/z/networks/n"
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/networks");
    let query = &requests[0].query;
    assert!(
        query.contains(
            "firewall=%2F%2Fscratch.origincloudapis.eu%2Fprojects%2Fp%2Fzones%2Fz%2Ffirewalls%2Ff"
        ),
        "{query}"
    );
    assert!(query.contains("owner=people%2Fada"), "{query}");
}

/// The proto JSON mapping of `bytes`: standard base64.
fn base64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Bytes no text reading would keep: invalid UTF-8, a NUL, and a final
/// newline.
const RAW: &[u8] = b"\xff\xfe\x00signed\r\n";

/// A `bytes` field is `--{field}-file`, read byte for byte from the file or
/// from standard input with `-`, and sent as the proto JSON mapping says, in
/// base64; one with explicit presence is left out when not given, and a
/// oneof's member is one of the oneof's flags. A response's bytes are printed
/// as the API's JSON has them.
#[tokio::test(flavor = "multi_thread")]
async fn a_bytes_field_is_read_from_a_file_byte_for_byte() {
    const SIGNED: &str = r#"{"signature":"c2lnbmF0dXJl"}"#;
    let server = recording(|_: &Received| Response::json(SIGNED)).await;
    let data = server.home.path().join("data");
    std::fs::write(&data, RAW).unwrap();
    let data = data.to_str().unwrap();
    let sign = |extra: &[&'static str]| {
        [
            &["sign-firewall-data", "--project", "p", "--zone", "z"][..],
            &["--firewall", "f", "--data-file"],
            extra,
        ]
        .concat()
    };

    let output = scratch(&server, &[&sign(&[])[..], &[data]].concat()).await;
    assert_eq!(success(&output), json!({"signature": "c2lnbmF0dXJl"}));
    let output = server
        .run_with_stdin(
            "scratch",
            &[
                &sign(&[])[..],
                &[data, "--salt-file", "-", "--label-bytes-file", data],
            ]
            .concat(),
            b"salt\n",
        )
        .await;
    success(&output);
    let output = server
        .run_with_stdin("scratch", &[&sign(&[])[..], &["-"]].concat(), RAW)
        .await;
    success(&output);

    let requests = server.requests();
    assert_eq!(requests.len(), 3, "{requests:?}");
    for r in &requests {
        assert_eq!(r.method, "POST");
        assert_eq!(r.path, "/v1/projects/p/zones/z/firewalls/f:signData");
    }
    const NAME: &str = "projects/p/zones/z/firewalls/f";
    assert_eq!(
        body(&requests[0]),
        json!({"name": NAME, "data": base64(RAW)})
    );
    assert_eq!(
        body(&requests[1]),
        json!({"name": NAME, "data": base64(RAW), "salt": base64(b"salt\n"), "labelBytes": base64(RAW)})
    );
    assert_eq!(
        body(&requests[2]),
        json!({"name": NAME, "data": base64(RAW)})
    );

    // An empty file is sent as such where the field has explicit presence.
    let empty = server.home.path().join("empty");
    std::fs::write(&empty, b"").unwrap();
    let output = scratch(
        &server,
        &[
            &sign(&[])[..],
            &[data, "--salt-file", empty.to_str().unwrap()],
        ]
        .concat(),
    )
    .await;
    success(&output);
    assert_eq!(body(&server.requests()[3])["salt"], "");

    // A missing file fails before anything is sent; so do two members of
    // the oneof, and no data at all.
    let missing = server.home.path().join("no-such-file");
    let output = scratch(
        &server,
        &[&sign(&[])[..], &[missing.to_str().unwrap()]].concat(),
    )
    .await;
    assert!(failure(&output).contains("no-such-file"), "{output:?}");
    refused(
        &server,
        &[
            &sign(&[])[..],
            &[data, "--label-text", "x", "--label-bytes-file", data],
        ]
        .concat(),
        "cannot be used with",
    )
    .await;
    refused(
        &server,
        &[
            "sign-firewall-data",
            "--project",
            "p",
            "--zone",
            "z",
            "--firewall",
            "f",
        ],
        "--data-file",
    )
    .await;
    assert_eq!(server.requests().len(), 4);
}

/// A resource's `bytes` field is `--{field}-file` in a create and in an
/// update, which names it in the mask; `--no-{field}` unsets one with
/// explicit presence.
#[tokio::test(flavor = "multi_thread")]
async fn a_resources_bytes_field_is_set_from_a_file() {
    let server = recording(echo).await;
    let certificate = server.home.path().join("certificate.der");
    std::fs::write(&certificate, RAW).unwrap();
    let certificate = certificate.to_str().unwrap();

    let output = scratch(
        &server,
        &[
            &CREATE[..],
            &["--rule", "protocol=TCP", "--certificate-file", certificate],
        ]
        .concat(),
    )
    .await;
    // The API's JSON, as the server answered it.
    assert_eq!(success(&output)["certificate"], base64(RAW));
    let created = body(&server.requests()[0]);
    assert_eq!(created["certificate"], base64(RAW));
    assert!(created.get("banner").is_none(), "{created}");

    let update = |extra: &[&'static str]| [&["update-firewall"][..], &FIREWALL, extra].concat();
    success(
        &scratch(
            &server,
            &[
                &update(&[])[..],
                &[
                    "--certificate-file",
                    certificate,
                    "--banner-file",
                    certificate,
                ],
            ]
            .concat(),
        )
        .await,
    );
    success(&scratch(&server, &update(&["--no-banner"])).await);
    let requests = server.requests();
    let mask = |r: &Received| {
        let mut paths: Vec<String> = r
            .query_values("updateMask")
            .iter()
            .flat_map(|v| v.split(',').map(str::to_owned).collect::<Vec<_>>())
            .collect();
        paths.sort();
        paths
    };
    assert_eq!(mask(&requests[1]), ["banner", "certificate"]);
    assert_eq!(body(&requests[1])["certificate"], base64(RAW));
    assert_eq!(body(&requests[1])["banner"], base64(RAW));
    assert_eq!(mask(&requests[2]), ["banner"]);
    assert!(body(&requests[2]).get("banner").is_none(), "{requests:?}");

    refused(
        &server,
        &[
            &update(&[])[..],
            &["--banner-file", certificate, "--no-banner"],
        ]
        .concat(),
        "cannot be used with",
    )
    .await;
    assert_eq!(server.requests().len(), 3);
}

/// A `bytes` field's help says where its flag reads from, and how; an
/// update's, that it is left as it is, and how to unset it.
#[test]
fn the_help_of_a_bytes_field() {
    let root = origincloud_cli::command();
    let group = root.find_subcommand("scratch").unwrap();
    let help = |command: &str, flag: &str| -> String {
        let arg = group
            .find_subcommand(command)
            .unwrap()
            .get_arguments()
            .find(|a| a.get_long() == Some(flag))
            .unwrap_or_else(|| panic!("{command} has no --{flag}"));
        assert_eq!(
            arg.get_value_names().map(|v| v.to_vec()),
            Some(vec!["PATH".into()]),
            "{command} --{flag}"
        );
        arg.get_long_help()
            .or(arg.get_help())
            .map(ToString::to_string)
            .unwrap_or_default()
    };
    let reads = "or from standard input with `-`, byte for byte.";
    assert!(help("sign-firewall-data", "data-file").contains(reads));
    assert!(help("sign-firewall-data", "salt-file").contains("Unset when not given."));
    assert!(help("sign-firewall-data", "label-bytes-file")
        .contains("One of `--label-text`, `--label-bytes-file`"));
    assert!(help("create-firewall", "certificate-file").contains(reads));
    let banner = help("update-firewall", "banner-file");
    assert!(
        banner.contains("Left as it is when not given. `--no-banner` unsets it."),
        "{banner}"
    );
}

/// A soft delete, one the service's `Undelete{Resource}` restores, needs
/// `--yes` like any delete, says what restores it, and prints the deleted
/// resource as the API answers it.
#[tokio::test(flavor = "multi_thread")]
async fn a_soft_delete_says_what_restores_it() {
    let server = recording(|_: &Received| {
        Response::json(r#"{"name":"projects/p/zones/z/firewalls/f","ports":[443]}"#)
    })
    .await;
    let delete = [&["delete-firewall"][..], &FIREWALL].concat();
    let refused = failure(&scratch(&server, &delete).await);
    assert!(
        refused.contains("`undelete-firewall` restores it until it is purged."),
        "{refused}"
    );
    assert!(server.requests().is_empty(), "{:?}", server.requests());

    let output = scratch(&server, &[&delete[..], &["--yes"]].concat()).await;
    assert_eq!(
        success(&output),
        json!({"name": "projects/p/zones/z/firewalls/f", "ports": [443]})
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(requests[0].path, "/v1/projects/p/zones/z/firewalls/f");
}
