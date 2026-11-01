//! The location service's commands and the operation commands against a
//! local server that records every request: the paths they call, the JSON
//! they print, and the names and values they refuse before sending
//! anything.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use common::{failure, recording, stdout, success};
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

/// Without `--timeout`, `wait-operation` asks until the operation is done
/// and prints it done; with one, it asks once, sending it, and prints the
/// operation as it is then.
#[tokio::test(flavor = "multi_thread")]
async fn wait_operation_asks_until_done_unless_given_a_timeout() {
    let answered = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&answered);
    // Running for the first two wait calls, done from the third.
    let server = recording(move |r: &Received| {
        assert_eq!(r.method, "POST");
        assert_eq!(r.path, "/v1beta1/projects/p/zones/z/operations/op-1:wait");
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
    let requests = server.requests();
    assert_eq!(requests.len(), 3, "{requests:?}");
    for r in &requests {
        let body: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert!(body.get("timeout").is_none(), "{body}");
        assert_eq!(body["name"], OPERATION, "{body}");
    }

    answered.store(0, Ordering::SeqCst);
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
    assert_eq!(printed, json(RUNNING), "one call, the operation as it is");
    let requests = server.requests();
    assert_eq!(requests.len(), 4, "{requests:?}");
    let body: serde_json::Value = serde_json::from_slice(&requests[3].body).unwrap();
    assert_eq!(body["timeout"], "90s", "{body}");
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
