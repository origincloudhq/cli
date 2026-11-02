//! The location service's commands and the operation commands against a
//! local server that records every request: the paths they call, the JSON
//! they print, and the names and values they refuse before sending
//! anything; and what the other groups' commands add to it: the default of
//! a regional name's `--region`, and the notice of a secret shown once.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use common::{failure, origin, recording, stdout, success};
use fake_server::{Received, Response};

const ZONES: &str = r#"{"zones":[
    {"name":"zones/z-a","displayName":"Zone A","region":"regions/r","services":["pvs"]},
    {"name":"zones/z-b","region":"regions/r"}]}"#;
const ZONE: &str =
    r#"{"name":"zones/z-a","displayName":"Zone A","region":"regions/r","services":["pvs"]}"#;
const REGIONS: &str = r#"{"regions":[{"name":"regions/r","displayName":"Region R","zones":["zones/z-a","zones/z-b"]}]}"#;
const REGION: &str =
    r#"{"name":"regions/r","displayName":"Region R","zones":["zones/z-a","zones/z-b"]}"#;

/// The location service as the server answers it.
fn location(request: &Received) -> Response {
    match request.path.as_str() {
        "/v1beta1/zones" => Response::json(ZONES),
        "/v1beta1/zones/z-a" => Response::json(ZONE),
        "/v1beta1/regions" => Response::json(REGIONS),
        "/v1beta1/regions/r" => Response::json(REGION),
        _ => Response::json(
            r#"{"error":{"code":404,"status":"NOT_FOUND","message":"no such thing"}}"#,
        )
        .with_status(404),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_location_commands_print_the_api_json() {
    let server = recording(location).await;

    let zones = success(&server.run("location", &["list-zones"]).await);
    assert_eq!(
        zones,
        serde_json::from_str::<serde_json::Value>(ZONES).unwrap()
    );
    let regions = success(&server.run("location", &["list-regions"]).await);
    assert_eq!(
        regions,
        serde_json::from_str::<serde_json::Value>(REGIONS).unwrap()
    );

    // By id and by whole name; no project or zone is needed, from a flag
    // or the profile.
    for zone in ["z-a", "zones/z-a"] {
        let printed = success(&server.run("location", &["get-zone", "--zone", zone]).await);
        assert_eq!(printed["displayName"], "Zone A", "{zone}");
        assert_eq!(printed["services"], serde_json::json!(["pvs"]));
    }
    let printed = success(
        &server
            .run("location", &["get-region", "--region", "r"])
            .await,
    );
    assert_eq!(
        printed["zones"],
        serde_json::json!(["zones/z-a", "zones/z-b"])
    );

    let calls: Vec<(String, String)> = server
        .requests()
        .iter()
        .map(|r| (r.method.clone(), r.path.clone()))
        .collect();
    let get = |p: &str| ("GET".to_owned(), p.to_owned());
    assert_eq!(
        calls,
        [
            get("/v1beta1/zones"),
            get("/v1beta1/regions"),
            get("/v1beta1/zones/z-a"),
            get("/v1beta1/zones/z-a"),
            get("/v1beta1/regions/r"),
        ]
    );
    assert!(server
        .requests()
        .iter()
        .all(|r| r.header("authorization") == Some("Bearer stored-access")));

    // The API's error, as the failure envelope.
    let description = failure(&server.run("location", &["get-zone", "--zone", "z-x"]).await);
    assert!(description.contains("no such thing"), "{description}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_catalog_prints_an_empty_array() {
    let server = recording(|_| Response::json("{}")).await;
    assert_eq!(
        success(&server.run("location", &["list-zones"]).await),
        serde_json::json!({ "zones": [] })
    );
    assert_eq!(
        success(&server.run("location", &["list-regions"]).await),
        serde_json::json!({ "regions": [] })
    );
}

/// A zone or region that could step out of its place in the path is
/// refused before any request, from the flag or a whole name.
#[tokio::test(flavor = "multi_thread")]
async fn a_location_name_outside_the_id_alphabet_is_refused() {
    let server = recording(location).await;
    for args in [
        &["get-zone", "--zone", ".."][..],
        &["get-zone", "--zone", "."],
        &["get-zone", "--zone", "zones/.."],
        &["get-zone", "--zone", "regions/r"],
        &["get-region", "--region", "a/b"],
        &["get-region", "--region", "r?x"],
    ] {
        let description = failure(&server.run("location", args).await);
        assert!(
            description.contains("isn't an id") || description.contains("isn't a name"),
            "{args:?}: {description}"
        );
    }
    assert!(server.requests().is_empty(), "{:?}", server.requests());
}

const RUNNING: &str = r#"{"name":"projects/p/zones/z/operations/op-1","metadata":{"@type":"type.googleapis.com/origincloud.longrunning.v1beta1.OperationMetadata","target":"projects/p/zones/z/volumes/v","verb":"create"}}"#;
const DONE: &str = r#"{"name":"projects/p/zones/z/operations/op-1","metadata":{"@type":"type.googleapis.com/origincloud.longrunning.v1beta1.OperationMetadata","target":"projects/p/zones/z/volumes/v","verb":"create"},"done":true,"response":{"@type":"type.googleapis.com/origincloud.pvs.v1beta1.Volume","name":"projects/p/zones/z/volumes/v","sizeGib":8}}"#;
const OPERATION: &str = "projects/p/zones/z/operations/op-1";

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap()
}

/// `get-operation` reads the operation `create-volume --no-wait` printed,
/// by its whole name or by its id with `--project` and `--zone`, from the
/// service's own host, and prints it as the API sends it.
#[tokio::test(flavor = "multi_thread")]
async fn get_operation_reads_the_operation_no_wait_printed() {
    let server = recording(|r: &Received| match (r.method.as_str(), r.path.as_str()) {
        ("POST", "/v1beta1/projects/p/zones/z/volumes") => Response::json(RUNNING),
        ("GET", "/v1beta1/projects/p/zones/z/operations/op-1") => Response::json(DONE),
        _ => Response::json(r#"{"error":{"code":404,"status":"NOT_FOUND","message":"gone"}}"#)
            .with_status(404),
    })
    .await;

    let started = success(
        &server
            .pvs(&[
                "create-volume",
                "--project",
                "p",
                "--zone",
                "z",
                "--size-gib",
                "8",
                "--storage-class",
                "gp1",
                "--no-wait",
            ])
            .await,
    );
    assert_eq!(started, json(RUNNING));
    let name = started["name"].as_str().unwrap();

    let by_name = success(&server.pvs(&["get-operation", "--operation", name]).await);
    assert_eq!(by_name, json(DONE));
    let by_id = success(
        &server
            .pvs_with(
                &["get-operation", "--operation", "op-1", "--zone", "z"],
                &[("ORIGINCLOUD_PROJECT", "p")],
            )
            .await,
    );
    assert_eq!(by_id, json(DONE));

    let requests = server.requests();
    assert_eq!(requests.len(), 3, "{requests:?}");
    for r in &requests[1..] {
        assert_eq!(r.method, "GET");
        assert_eq!(r.path, "/v1beta1/projects/p/zones/z/operations/op-1");
    }

    let description = failure(
        &server
            .pvs(&[
                "get-operation",
                "--operation",
                "projects/p/zones/z/operations/op-9",
            ])
            .await,
    );
    assert!(description.contains("gone"), "{description}");
}

/// The wait calls the server received, and the timeout each asked for.
fn wait_calls(server: &common::Recording) -> Vec<Option<String>> {
    server
        .requests()
        .iter()
        .map(|r| {
            assert_eq!(r.method, "POST");
            assert_eq!(r.path, "/v1beta1/projects/p/zones/z/operations/op-1:wait");
            let body: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
            assert_eq!(body["name"], OPERATION, "{body}");
            body.get("timeout").map(|t| t.as_str().unwrap().to_owned())
        })
        .collect()
}

/// Without `--timeout`, `wait-operation` asks until the operation is done,
/// each call leaving the API's own wait of a minute, and prints it done.
#[tokio::test(flavor = "multi_thread")]
async fn wait_operation_asks_until_done() {
    let answered = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&answered);
    // Running for the first two wait calls, done from the third.
    let server = recording(move |_: &Received| {
        if count.fetch_add(1, Ordering::SeqCst) < 2 {
            Response::json(RUNNING)
        } else {
            Response::json(DONE)
        }
    })
    .await;

    let printed = success(
        &server
            .pvs(&["wait-operation", "--operation", OPERATION])
            .await,
    );
    assert_eq!(printed, json(DONE));
    assert_eq!(wait_calls(&server), [None, None, None]);
}

/// `--timeout` is the command's deadline: each call asks the API to wait
/// what is left of it, a minute at most, which is all the API waits.
#[tokio::test(flavor = "multi_thread")]
async fn wait_operation_asks_for_at_most_a_minute_at_a_time() {
    let server = recording(|_: &Received| Response::json(DONE)).await;
    let printed = success(
        &server
            .pvs(&[
                "wait-operation",
                "--project",
                "p",
                "--zone",
                "z",
                "--operation",
                "op-1",
                "--timeout",
                "1m30s",
            ])
            .await,
    );
    assert_eq!(printed, json(DONE));
    assert_eq!(wait_calls(&server), [Some("60s".to_owned())]);
}

/// When `--timeout` passes before the operation is done, the command
/// prints it as it is and fails.
#[tokio::test(flavor = "multi_thread")]
async fn wait_operation_fails_when_its_timeout_passes() {
    let server = recording(|_: &Received| Response::json(RUNNING)).await;
    let started = std::time::Instant::now();
    let output = server
        .pvs(&[
            "wait-operation",
            "--operation",
            OPERATION,
            "--timeout",
            "2s",
        ])
        .await;
    let elapsed = started.elapsed();
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    assert_eq!(stdout(&output), json(RUNNING));
    assert!(
        elapsed >= std::time::Duration::from_secs(2)
            && elapsed < std::time::Duration::from_secs(10),
        "{elapsed:?}"
    );
    let calls = wait_calls(&server);
    assert!(calls.len() >= 2 && calls.len() <= 4, "{calls:?}");
    assert_eq!(calls[0].as_deref(), Some("2s"));
}

const FAILED: &str = r#"{"name":"projects/p/zones/z/operations/op-1","done":true,"error":{"code":8,"message":"the zone has no capacity left"}}"#;

/// A failed operation is printed, its error included, by both commands;
/// `wait-operation` then fails, as a command waiting for its own operation
/// does, and `get-operation`, which only reads it, doesn't.
#[tokio::test(flavor = "multi_thread")]
async fn wait_operation_fails_with_a_failed_operation() {
    let server = recording(|_: &Received| Response::json(FAILED)).await;
    let output = server
        .pvs(&["wait-operation", "--operation", OPERATION])
        .await;
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    let printed = stdout(&output);
    assert_eq!(printed, json(FAILED));
    assert_eq!(printed["error"]["message"], "the zone has no capacity left");

    let printed = success(
        &server
            .pvs(&["get-operation", "--operation", OPERATION])
            .await,
    );
    assert_eq!(printed, json(FAILED));

    // `create-volume` waiting for the same failed operation fails too.
    let description = common::failure(
        &server
            .pvs(&[
                "create-volume",
                "--project",
                "p",
                "--zone",
                "z",
                "--size-gib",
                "8",
                "--storage-class",
                "gp1",
            ])
            .await,
    );
    assert!(description.contains("no capacity left"), "{description}");
}

/// A value `--timeout` can't read, or an operation name that could step
/// out of its place in the path, is refused before any request.
#[tokio::test(flavor = "multi_thread")]
async fn operation_commands_refuse_bad_names_and_durations() {
    let server = recording(|_| Response::json(DONE)).await;
    for timeout in ["30", "1.5s", "-1s", "1d", "s"] {
        let output = server
            .pvs(&[
                "wait-operation",
                "--operation",
                OPERATION,
                "--timeout",
                timeout,
            ])
            .await;
        assert_eq!(
            output.status.code(),
            Some(2),
            "{timeout}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let description = failure(
        &server
            .pvs(&[
                "wait-operation",
                "--operation",
                OPERATION,
                "--timeout",
                "0s",
            ])
            .await,
    );
    assert!(description.contains("longer than zero"), "{description}");
    for command in ["get-operation", "wait-operation"] {
        for operation in [
            "..",
            ".",
            "projects/p/zones/../operations/op-1",
            "projects/p/zones/z/volumes/v",
            "op-1/..",
        ] {
            let description = failure(
                &server
                    .pvs(&[
                        command,
                        "--project",
                        "p",
                        "--zone",
                        "z",
                        "--operation",
                        operation,
                    ])
                    .await,
            );
            assert!(
                description.contains("isn't an id") || description.contains("isn't a name"),
                "{command} {operation}: {description}"
            );
        }
        // An id needs its project and zone, from a flag or the profile.
        let description = failure(&server.pvs(&[command, "--operation", "op-1"]).await);
        assert!(
            description.contains("--project is required"),
            "{description}"
        );
    }
    assert!(server.requests().is_empty(), "{:?}", server.requests());
}

/// The location group takes `--endpoint` under the same rule as every
/// generated group.
#[tokio::test(flavor = "multi_thread")]
async fn the_location_endpoint_is_https_or_this_machine() {
    let server = recording(location).await;
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_origin"))
        .args(["location", "--endpoint", "http://192.0.2.1:9", "list-zones"])
        .env_clear()
        .env("HOME", server.home.path())
        .output()
        .unwrap();
    let description = failure(&output);
    assert!(
        description.contains("http:// is only for this machine"),
        "{description}"
    );
    let output = server.run("location", &["list-zones"]).await;
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("instead of the service's own host"),
        "the endpoint override is said on stderr"
    );
    let _ = stdout(&output);
}

/// Each group's `--endpoint` variable redirects that group alone: the pvs
/// one leaves `origin location` at its own, and the shared variable of
/// earlier versions redirects nothing.
#[tokio::test(flavor = "multi_thread")]
async fn each_groups_endpoint_variable_redirects_that_group_alone() {
    let pvs = recording(|_: &Received| {
        Response::json(r#"{"name":"projects/p/zones/z/volumes/v","sizeGib":8}"#)
    })
    .await;
    let location = recording(location).await;
    let (pvs_url, location_url) = (pvs.server.url(), location.server.url());
    let env = [
        ("ORIGINCLOUD_PVS_ENDPOINT", pvs_url.as_str()),
        ("ORIGINCLOUD_LOCATION_ENDPOINT", location_url.as_str()),
        ("ORIGINCLOUD_ENDPOINT", "http://127.0.0.1:9"),
    ];
    let home = pvs.home.path();

    let output = origin(home, &["location", "list-zones"], &env).await;
    assert_eq!(success(&output), json(ZONES));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!(
            "Sending the `origin location` calls to {location_url}, from --endpoint or ORIGINCLOUD_LOCATION_ENDPOINT"
        )),
        "{stderr}"
    );
    let output = origin(
        home,
        &[
            "pvs",
            "get-volume",
            "--volume",
            "projects/p/zones/z/volumes/v",
        ],
        &env,
    )
    .await;
    assert_eq!(success(&output)["sizeGib"], 8);

    assert_eq!(location.requests().len(), 1);
    assert_eq!(location.requests()[0].path, "/v1beta1/zones");
    assert_eq!(pvs.requests().len(), 1);
    assert_eq!(
        pvs.requests()[0].path,
        "/v1beta1/projects/p/zones/z/volumes/v"
    );

    // `--endpoint` given to one group isn't an option of the other's.
    let output = origin(
        home,
        &["--endpoint", &pvs_url, "location", "list-zones"],
        &[],
    )
    .await;
    assert_eq!(output.status.code(), Some(2));
}

/// A list given no `--zone` looks up the zones at the location service, not
/// at the pvs `--endpoint`: at the location group's own endpoint when its
/// variable names one, and says where.
#[tokio::test(flavor = "multi_thread")]
async fn the_zone_lookup_goes_to_the_location_service_not_the_pvs_endpoint() {
    let pvs = recording(|r: &Received| match r.path.as_str() {
        "/v1beta1/projects/p/zones/z-a/volumes" => {
            Response::json(r#"{"volumes":[{"name":"projects/p/zones/z-a/volumes/v-a"}]}"#)
        }
        _ => Response::json(r#"{"error":{"code":404,"status":"NOT_FOUND","message":"not pvs"}}"#)
            .with_status(404),
    })
    .await;
    let location = recording(location).await;
    let location_url = location.server.url();
    let output = pvs
        .pvs_with(
            &["list-volumes", "--project", "p"],
            &[("ORIGINCLOUD_LOCATION_ENDPOINT", &location_url)],
        )
        .await;
    let printed = success(&output);
    assert_eq!(
        printed["volumes"],
        serde_json::json!([{ "name": "projects/p/zones/z-a/volumes/v-a" }])
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!(
            "Looking up the zones at {location_url}, from ORIGINCLOUD_LOCATION_ENDPOINT"
        )),
        "{stderr}"
    );
    let location_paths: Vec<String> = location.requests().iter().map(|r| r.path.clone()).collect();
    assert_eq!(location_paths, ["/v1beta1/zones"]);
    let pvs_paths: Vec<String> = pvs.requests().iter().map(|r| r.path.clone()).collect();
    assert_eq!(pvs_paths, ["/v1beta1/projects/p/zones/z-a/volumes"]);

    // An invalid location endpoint is refused before any request.
    let description = failure(
        &pvs.pvs_with(
            &["list-volumes", "--project", "p"],
            &[("ORIGINCLOUD_LOCATION_ENDPOINT", "http://192.0.2.1:9")],
        )
        .await,
    );
    assert!(
        description.contains("ORIGINCLOUD_LOCATION_ENDPOINT"),
        "{description}"
    );
    assert_eq!(pvs.requests().len(), 1);
}

/// A regional name's `--region` is the flag, else `ORIGINCLOUD_REGION`, else
/// the profile's `region` (`origin config profile-set region`), else the
/// region of the default zone; without any, the refusal names each way of
/// giving one, before any request.
#[tokio::test(flavor = "multi_thread")]
async fn a_regional_name_takes_the_flag_then_the_environment_then_the_profile() {
    let server = recording(|_: &Received| Response::json(r#"{"vpcs":[]}"#)).await;
    let list = |args: &'static [&'static str], env: &'static [(&'static str, &'static str)]| {
        let mut all = vec!["list-vpcs", "--project", "p"];
        all.extend(args);
        let server = &server;
        async move { server.run_with("network", &all, env).await }
    };
    let set = |key: &'static str, value: &'static str| {
        let home = server.home.path().to_owned();
        async move {
            success(&origin(&home, &["config", "profile-set", key, value], &[]).await);
        }
    };

    let description = failure(&list(&[], &[]).await);
    for part in [
        "--region is required",
        "ORIGINCLOUD_REGION",
        "`origin config profile-set region <region>`",
    ] {
        assert!(description.contains(part), "{part}: {description}");
    }
    assert!(server.requests().is_empty());

    set("default-zone", "eu-central1-az1").await;
    success(&list(&[], &[]).await);
    set("region", "eu-central2").await;
    success(&list(&[], &[]).await);
    success(&list(&[], &[("ORIGINCLOUD_REGION", "eu-central3")]).await);
    success(
        &list(
            &["--region", "eu-central4"],
            &[("ORIGINCLOUD_REGION", "eu-central3")],
        )
        .await,
    );

    let paths: Vec<String> = server.requests().iter().map(|r| r.path.clone()).collect();
    assert_eq!(
        paths,
        [
            "/v1beta1/projects/p/regions/eu-central1/vpcs",
            "/v1beta1/projects/p/regions/eu-central2/vpcs",
            "/v1beta1/projects/p/regions/eu-central3/vpcs",
            "/v1beta1/projects/p/regions/eu-central4/vpcs",
        ]
    );
}

/// A generated private key is printed in the JSON as the API sends it, and
/// standard error says it is shown this once and how to keep it; an
/// imported key, whose response has none, says nothing.
#[tokio::test(flavor = "multi_thread")]
async fn a_generated_private_key_comes_with_a_notice() {
    const GENERATED: &str = r#"{"name":"projects/p/keyPairs/k","publicKey":"ssh-ed25519 AAAA","privateKey":"-----BEGIN OPENSSH PRIVATE KEY-----"}"#;
    const IMPORTED: &str = r#"{"name":"projects/p/keyPairs/k","publicKey":"ssh-ed25519 AAAA"}"#;
    let server = recording(|r: &Received| {
        if String::from_utf8_lossy(&r.body).contains("publicKey") {
            Response::json(IMPORTED)
        } else {
            Response::json(GENERATED)
        }
    })
    .await;

    let output = server
        .run("compute", &["create-key-pair", "--project", "p"])
        .await;
    assert_eq!(success(&output), json(GENERATED));
    let stderr = String::from_utf8_lossy(&output.stderr);
    for part in [
        "`privateKey`",
        "this once",
        "umask 077",
        "jq -r .privateKey",
    ] {
        assert!(stderr.contains(part), "{part}: {stderr}");
    }
    assert!(!stderr.contains("BEGIN OPENSSH"), "{stderr}");

    let output = server
        .run(
            "compute",
            &[
                "create-key-pair",
                "--project",
                "p",
                "--public-key",
                "ssh-ed25519 AAAA",
            ],
        )
        .await;
    assert_eq!(success(&output), json(IMPORTED));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("privateKey"), "{stderr}");
}
