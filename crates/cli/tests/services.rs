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
/// refused before any request, from the flag or a whole name; any other
/// id is the API's to judge, and goes to it escaped, as the SDK sends it.
#[tokio::test(flavor = "multi_thread")]
async fn a_location_name_that_could_leave_its_place_is_refused() {
    let server = recording(location).await;
    for args in [
        &["get-zone", "--zone", ".."][..],
        &["get-zone", "--zone", "."],
        &["get-zone", "--zone", ""],
        &["get-zone", "--zone", "zones/.."],
        &["get-zone", "--zone", "regions/r"],
        &["get-region", "--region", "a/b"],
    ] {
        let description = failure(&server.run("location", args).await);
        assert!(
            description.contains("isn't an id") || description.contains("isn't a name"),
            "{args:?}: {description}"
        );
    }
    assert!(server.requests().is_empty(), "{:?}", server.requests());

    for region in ["r?x", "regions/r?x"] {
        failure(
            &server
                .run("location", &["get-region", "--region", region])
                .await,
        );
    }
    let paths: Vec<String> = server.requests().iter().map(|r| r.path.clone()).collect();
    assert_eq!(paths, ["/v1beta1/regions/r%3Fx", "/v1beta1/regions/r%3Fx"]);
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
                "//pvs.origincloudapis.eu/storageClasses/gp1",
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
                "//pvs.origincloudapis.eu/storageClasses/gp1",
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
    for timeout in ["30", "1.5s", "-1s", "1w", "s"] {
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

/// A new key's credentials file, which holds its private key and which the
/// API returns this once, is written to the file `--credentials-file`
/// names, readable by its owner alone, and never printed: standard output
/// has the key without it, and standard error names the file and the key.
#[tokio::test(flavor = "multi_thread")]
async fn a_key_s_credentials_file_is_written_to_the_named_file_never_printed() {
    const SECRET: &str = r#"{"type":"service_account_key","private_key":"-----BEGIN PRIVATE KEY-----\nPRIVATE-KEY-MATERIAL\n-----END PRIVATE KEY-----\n"}"#;
    let created = serde_json::json!({
        "name": "organizations/o/serviceAccounts/sa-1/keys/k-1",
        "algorithm": "ES256",
        "state": "ACTIVE",
        "credentialsFile": SECRET,
    })
    .to_string();
    let server = recording(move |_: &Received| Response::json(&created)).await;
    let file = server.home.path().join("sa-1.json");
    let path = file.to_str().unwrap();
    let create = [
        "create-service-account-key",
        "--organization",
        "o",
        "--service-account",
        "sa-1",
        "--credentials-file",
        path,
    ];

    for format in ["json", "table"] {
        let _ = std::fs::remove_file(&file);
        let output = server
            .run("iam", &[&create[..], &["-o", format]].concat())
            .await;
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let printed = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        for said in [&printed, &stderr] {
            assert!(!said.contains("PRIVATE-KEY-MATERIAL"), "{said}");
            assert!(!said.contains("PRIVATE KEY"), "{said}");
        }
        assert!(
            printed.contains("organizations/o/serviceAccounts/sa-1/keys/k-1"),
            "{printed}"
        );
        assert!(stderr.contains(path), "the file: {stderr}");
        assert!(
            stderr.contains("organizations/o/serviceAccounts/sa-1/keys/k-1"),
            "the key: {stderr}"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), SECRET);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "{mode:o}");
        }
    }
    std::fs::remove_file(&file).unwrap();
    let printed = success(&server.run("iam", &create).await);
    assert_eq!(
        printed["name"],
        "organizations/o/serviceAccounts/sa-1/keys/k-1"
    );
    assert!(printed.get("credentialsFile").is_none(), "{printed}");
    assert_eq!(
        server.requests()[0].path,
        "/v1beta1/organizations/o/serviceAccounts/sa-1/keys"
    );
}

/// An existing file is refused before any request, and left as it is,
/// unless `--overwrite`, which replaces it once the response is in; a
/// request that fails leaves no new file behind and an existing one as it
/// was. `--credentials-file` is required, and isn't standard output.
#[tokio::test(flavor = "multi_thread")]
async fn a_key_s_credentials_file_never_replaces_a_file_unasked() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let server = recording(move |_: &Received| {
        if seen.fetch_add(1, Ordering::SeqCst) == 0 {
            Response::json(
                r#"{"name":"organizations/o/serviceAccounts/sa-1/keys/k-2","credentialsFile":"NEW-KEY"}"#,
            )
        } else {
            Response::json(r#"{"error":{"code":403,"status":"PERMISSION_DENIED","message":"denied"}}"#)
                .with_status(403)
        }
    })
    .await;
    let file = server.home.path().join("existing.json");
    std::fs::write(&file, "OLD-KEY").unwrap();
    let path = file.to_str().unwrap();
    let create = |extra: &'static [&'static str]| {
        let mut args = vec![
            "create-service-account-key",
            "--organization",
            "o",
            "--service-account",
            "sa-1",
        ];
        args.extend_from_slice(extra);
        args
    };

    let with_file = [&create(&[])[..], &["--credentials-file", path]].concat();
    let description = failure(&server.run("iam", &with_file).await);
    assert!(description.contains("--overwrite"), "{description}");
    assert!(description.contains(path), "{description}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "OLD-KEY");
    assert!(server.requests().is_empty(), "refused before any request");

    let output = server
        .run(
            "iam",
            &[&create(&[])[..], &["--credentials-file", "-"]].concat(),
        )
        .await;
    assert!(failure(&output).contains("never written to standard output"));
    let output = server.run("iam", &create(&[])).await;
    assert_eq!(output.status.code(), Some(2), "a usage error: {output:?}");
    assert!(server.requests().is_empty(), "refused before any request");

    let overwrite = [&with_file[..], &["--overwrite"]].concat();
    success(&server.run("iam", &overwrite).await);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "NEW-KEY");

    // The API refuses from here on.
    std::fs::write(&file, "OLD-KEY").unwrap();
    failure(&server.run("iam", &overwrite).await);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "OLD-KEY");
    let fresh = server.home.path().join("fresh.json");
    let fresh_path = fresh.to_str().unwrap();
    failure(
        &server
            .run(
                "iam",
                &[&create(&[])[..], &["--credentials-file", fresh_path]].concat(),
            )
            .await,
    );
    assert!(!fresh.exists(), "no file is left behind");
    let leftovers: Vec<_> = std::fs::read_dir(server.home.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

/// What a refusal's metadata names that its message doesn't is said under
/// the message, as a refused field is; a failure the API didn't answer has
/// none of the refusal's keys.
#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_s_metadata_the_message_doesn_t_name_is_said() {
    const REFUSED: &str = r#"{"error":{"code":409,"status":"FAILED_PRECONDITION","message":"the volume is in use","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"CONFLICT","domain":"origincloudapis.eu","metadata":{"instance":"projects/p/zones/z/instances/i-1"}}]}}"#;
    let server = recording(|_: &Received| Response::json(REFUSED).with_status(409)).await;
    let delete = [
        "delete-volume",
        "--project",
        "p",
        "--zone",
        "z",
        "--volume",
        "v1",
        "--yes",
    ];

    let output = server
        .pvs(&[&delete[..], &["--output", "table"]].concat())
        .await;
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("the volume is in use\n  instance: projects/p/zones/z/instances/i-1"),
        "{stderr}"
    );

    let output = server.pvs(&delete).await;
    let printed = stdout(&output);
    assert_eq!(
        printed["metadata"],
        json(r#"{"instance": "projects/p/zones/z/instances/i-1"}"#),
        "{printed}"
    );
    assert!(printed.get("requestId").is_none(), "{printed}");
}

/// `create-instance` builds its own parent from `--project` and `--zone`,
/// and sends each reference as the full name given, on the host of the
/// referenced type's service, of another project too, and an instance type
/// whose id holds dots. A reference's parts never fill in the instance's,
/// nor are they checked against them.
#[tokio::test(flavor = "multi_thread")]
async fn create_instance_sends_references_as_full_names() {
    const LAUNCHING: &str = r#"{"name":"projects/p/zones/z/operations/op-1"}"#;
    const TYPE: &str = "//compute.origincloudapis.eu/instanceTypes/gp1.core.2";
    const IMAGE: &str = "//compute.origincloudapis.eu/projects/p/zones/z/images/img-1";
    const KEY_PAIR: &str = "//compute.origincloudapis.eu/projects/p/keyPairs/k";
    let server = recording(|_| Response::json(LAUNCHING)).await;
    let create = |instance_type: &'static str, image: &'static str, key_pair: &'static str| {
        vec![
            "create-instance",
            "--project",
            "p",
            "--zone",
            "z",
            "--instance-type",
            instance_type,
            "--image",
            image,
            "--key-pair",
            key_pair,
            "--network-interface",
            "//network.origincloudapis.eu/projects/p/zones/z/networkInterfaces/ni-1",
            "--no-wait",
        ]
    };
    for (instance_type, image, key_pair) in [
        (TYPE, IMAGE, KEY_PAIR),
        // Another project's key pair, and a public image of another zone:
        // the server decides whether they may be used.
        (
            TYPE,
            "//compute.origincloudapis.eu/zones/z2/images/img-1",
            "//compute.origincloudapis.eu/projects/other/keyPairs/k",
        ),
    ] {
        let args = create(instance_type, image, key_pair);
        let printed = success(&server.run("compute", &args).await);
        assert_eq!(printed, json(LAUNCHING), "{args:?}");
        let requests = server.requests();
        let sent = requests.last().unwrap();
        assert_eq!(sent.method, "POST", "{args:?}");
        assert_eq!(
            sent.path, "/v1beta1/projects/p/zones/z/instances",
            "{args:?}"
        );
        let body: serde_json::Value = serde_json::from_slice(&sent.body).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "instanceType": instance_type,
                "image": image,
                "keyPair": key_pair,
                "networkInterfaces": ["//network.origincloudapis.eu/projects/p/zones/z/networkInterfaces/ni-1"],
            }),
            "{args:?}"
        );
    }

    // A relative name or an id is refused, whatever `--project` and
    // `--zone` say, naming the full form; so is a full name on another
    // service's host, naming the expected one, and a name of no form the
    // type has. Nothing is sent.
    let before = server.requests().len();
    for (args, refusal) in [
        (
            create(TYPE, IMAGE, "k"),
            "--key-pair: \"k\" is an id; give the resource's full name, such as \
             //compute.origincloudapis.eu/projects/{project}/keyPairs/{key_pair}",
        ),
        (
            create(TYPE, IMAGE, "projects/p/keyPairs/k"),
            "--key-pair: \"projects/p/keyPairs/k\" is a relative name; give the resource's full \
             name, such as //compute.origincloudapis.eu/projects/{project}/keyPairs/{key_pair}",
        ),
        (
            create(TYPE, "zones/z/images/img-1", KEY_PAIR),
            "--image: \"zones/z/images/img-1\" is a relative name",
        ),
        (
            create(TYPE, "img-1", KEY_PAIR),
            "--image: \"img-1\" is an id",
        ),
        (
            create("gp1.core.2", IMAGE, KEY_PAIR),
            "--instance-type: \"gp1.core.2\" is an id",
        ),
        (
            create("instanceTypes/gp1.core.2", IMAGE, KEY_PAIR),
            "--instance-type: \"instanceTypes/gp1.core.2\" is a relative name",
        ),
        (
            create(
                TYPE,
                IMAGE,
                "//pvs.origincloudapis.eu/projects/p/keyPairs/k",
            ),
            "--key-pair: \"//pvs.origincloudapis.eu/projects/p/keyPairs/k\" is a name on \
             pvs.origincloudapis.eu, but --key-pair takes a name on compute.origincloudapis.eu",
        ),
        (
            create(TYPE, "//compute.origincloudapis.eu/images/img-1", KEY_PAIR),
            "isn't a name of the form projects/{project}/zones/{zone}/images/{image} or \
             zones/{zone}/images/{image}",
        ),
        (
            create(
                "//compute.origincloudapis.eu/instanceTypes/gp1.core.2/x",
                IMAGE,
                KEY_PAIR,
            ),
            "isn't a name of the form instanceTypes/{instance_type}",
        ),
    ] {
        let description = failure(&server.run("compute", &args).await);
        assert!(description.contains(refusal), "{args:?}: {description}");
    }
    assert_eq!(server.requests().len(), before, "{:?}", server.requests());
}

/// A volume attachment names its instance, a parent, by its id or its whole
/// name, which gives the project and zone, and a volume of another service
/// by its full name, sent as given. A disagreeing `--zone` is refused, and
/// so is a relative or bare volume, whatever `--project` and `--zone` say.
#[tokio::test(flavor = "multi_thread")]
async fn a_volume_attachment_takes_a_parent_and_another_services_volume() {
    const ATTACHING: &str = r#"{"name":"projects/p/zones/z/operations/op-1"}"#;
    let server = recording(|_| Response::json(ATTACHING)).await;
    let volume = "//pvs.origincloudapis.eu/projects/p/zones/z/volumes/vol-1";
    for instance in [
        "//compute.origincloudapis.eu/projects/p/zones/z/instances/i-1",
        "projects/p/zones/z/instances/i-1",
    ] {
        success(
            &server
                .run(
                    "compute",
                    &[
                        "create-volume-attachment",
                        "--instance",
                        instance,
                        "--volume",
                        volume,
                        "--no-wait",
                    ],
                )
                .await,
        );
    }
    success(
        &server
            .run(
                "compute",
                &[
                    "create-volume-attachment",
                    "--project",
                    "p",
                    "--zone",
                    "z",
                    "--instance",
                    "i-1",
                    "--volume",
                    volume,
                    "--no-wait",
                ],
            )
            .await,
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 3, "{requests:?}");
    for sent in &requests {
        assert_eq!(
            sent.path,
            "/v1beta1/projects/p/zones/z/instances/i-1/volumeAttachments"
        );
        let body: serde_json::Value = serde_json::from_slice(&sent.body).unwrap();
        assert_eq!(body["volume"], volume);
    }

    for (args, refusal) in [
        (
            vec![
                "create-volume-attachment",
                "--zone",
                "z2",
                "--instance",
                "projects/p/zones/z/instances/i-1",
                "--volume",
                volume,
            ],
            "--zone z2 differs from the zone of --instance projects/p/zones/z/instances/i-1",
        ),
        (
            vec![
                "create-volume-attachment",
                "--instance",
                "//pvs.origincloudapis.eu/projects/p/zones/z/instances/i-1",
                "--volume",
                volume,
            ],
            "--instance: \"//pvs.origincloudapis.eu/projects/p/zones/z/instances/i-1\" is a name \
             on pvs.origincloudapis.eu, but --instance takes a name on compute.origincloudapis.eu",
        ),
        (
            vec![
                "create-volume-attachment",
                "--project",
                "p",
                "--zone",
                "z",
                "--instance",
                "i-1",
                "--volume",
                "vol-1",
            ],
            "--volume: \"vol-1\" is an id; give the resource's full name, such as \
             //pvs.origincloudapis.eu/projects/{project}/zones/{zone}/volumes/{volume}",
        ),
        (
            vec![
                "create-volume-attachment",
                "--instance",
                "projects/p/zones/z/instances/i-1",
                "--volume",
                "projects/p/zones/z/volumes/vol-1",
            ],
            "--volume: \"projects/p/zones/z/volumes/vol-1\" is a relative name; give the \
             resource's full name, such as \
             //pvs.origincloudapis.eu/projects/{project}/zones/{zone}/volumes/{volume}",
        ),
    ] {
        let description = failure(&server.run("compute", &args).await);
        assert!(description.contains(refusal), "{args:?}: {description}");
    }
    assert_eq!(server.requests().len(), 3);
}

/// The command's own target takes its full name, and the request's path is
/// its relative name; one of another service is refused.
#[tokio::test(flavor = "multi_thread")]
async fn a_target_takes_its_full_name_and_sends_its_relative_name() {
    const VOLUME: &str = r#"{"name":"projects/p/zones/z/volumes/v"}"#;
    let server = recording(|_| Response::json(VOLUME)).await;
    let output = server
        .pvs(&[
            "get-volume",
            "--volume",
            "//pvs.origincloudapis.eu/projects/p/zones/z/volumes/v",
        ])
        .await;
    assert_eq!(success(&output), json(VOLUME));
    let requests = server.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0].path, "/v1beta1/projects/p/zones/z/volumes/v");

    let description = failure(
        &server
            .pvs(&[
                "get-volume",
                "--volume",
                "//compute.origincloudapis.eu/projects/p/zones/z/volumes/v",
            ])
            .await,
    );
    assert!(
        description.contains("but --volume takes a name on pvs.origincloudapis.eu"),
        "{description}"
    );
    assert_eq!(server.requests().len(), 1);
}

/// A security group rule's parent, its security group, is given by its
/// full name alone, with no `--project` or `--region`.
#[tokio::test(flavor = "multi_thread")]
async fn a_parent_given_by_its_full_name_needs_no_other_flags() {
    const CREATING: &str = r#"{"name":"projects/p/regions/r/operations/op-1"}"#;
    let server = recording(|_| Response::json(CREATING)).await;
    let peer = "//network.origincloudapis.eu/projects/other/regions/r/securityGroups/sg-2";
    success(
        &server
            .run(
                "network",
                &[
                    "create-security-group-rule",
                    "--security-group",
                    "//network.origincloudapis.eu/projects/p/regions/r/securityGroups/sg-1",
                    "--direction",
                    "INGRESS",
                    "--protocol",
                    "TCP",
                    "--peer-security-group",
                    peer,
                    "--no-wait",
                ],
            )
            .await,
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(
        requests[0].path,
        "/v1beta1/projects/p/regions/r/securityGroups/sg-1/rules"
    );
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["peerSecurityGroup"], peer);
}

/// `--organization` and `--project` take their resource's whole name too,
/// relative or full on resourcemanager's host, as the id they stand for;
/// one on another host is refused, and `--zone` stays an id.
#[tokio::test(flavor = "multi_thread")]
async fn an_organization_or_a_project_takes_its_own_whole_name() {
    let server = recording(|_| Response::json("{}")).await;
    for organization in [
        "o-1",
        "organizations/o-1",
        "//resourcemanager.origincloudapis.eu/organizations/o-1",
    ] {
        success(
            &server
                .run(
                    "iam",
                    &[
                        "list-audit-log-entries",
                        "--organization",
                        organization,
                        "--page-size",
                        "1",
                    ],
                )
                .await,
        );
    }
    success(
        &server
            .pvs(&[
                "list-volumes",
                "--project",
                "projects/p",
                "--zone",
                "z",
                "--page-size",
                "1",
            ])
            .await,
    );
    let paths: Vec<String> = server.requests().iter().map(|r| r.path.clone()).collect();
    assert_eq!(
        paths,
        [
            "/v1beta1/organizations/o-1/auditLogEntries",
            "/v1beta1/organizations/o-1/auditLogEntries",
            "/v1beta1/organizations/o-1/auditLogEntries",
            "/v1beta1/projects/p/zones/z/volumes",
        ]
    );

    let description = failure(
        &server
            .run(
                "iam",
                &[
                    "list-audit-log-entries",
                    "--organization",
                    "//iam.origincloudapis.eu/organizations/o-1",
                ],
            )
            .await,
    );
    assert!(
        description
            .contains("but --organization takes a name on resourcemanager.origincloudapis.eu"),
        "{description}"
    );
    let description = failure(
        &server
            .pvs(&[
                "list-volumes",
                "--project",
                "p",
                "--zone",
                "projects/p/zones/z",
            ])
            .await,
    );
    assert!(description.contains("isn't an id"), "{description}");
    assert_eq!(server.requests().len(), 4);
}

/// `assume-service-account` reads the OIDC token from a file, never from
/// the command line, and sends it without the file's final newline; a
/// missing file fails before anything is sent.
#[tokio::test(flavor = "multi_thread")]
async fn assume_service_account_reads_the_token_from_a_file() {
    const ASSUMED: &str = r#"{"accessToken":"issued","expireTime":"2026-11-11T20:00:00Z","serviceAccount":"organizations/o/serviceAccounts/s","federatedCredential":"organizations/o/serviceAccounts/s/federatedCredentials/f"}"#;
    let server = recording(|_: &Received| Response::json(ASSUMED)).await;
    let token = server.home.path().join("token");
    std::fs::write(&token, "job.token.value\n").unwrap();

    let output = server
        .run(
            "sts",
            &[
                "assume-service-account",
                "--oidc-token-file",
                token.to_str().unwrap(),
                "--service-account",
                "//iam.origincloudapis.eu/organizations/o/serviceAccounts/s",
                "--lifetime",
                "10m",
            ],
        )
        .await;
    assert_eq!(success(&output), json(ASSUMED));
    let requests = server.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0].path, "/v1beta1:assumeServiceAccount");
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(
        body,
        json(
            r#"{"oidcToken":"job.token.value","serviceAccount":"//iam.origincloudapis.eu/organizations/o/serviceAccounts/s","lifetime":"600s"}"#
        )
    );

    let missing = server.home.path().join("no-such-token");
    let output = server
        .run(
            "sts",
            &[
                "assume-service-account",
                "--oidc-token-file",
                missing.to_str().unwrap(),
            ],
        )
        .await;
    assert!(failure(&output).contains("no-such-token"), "{output:?}");
    assert_eq!(server.requests().len(), 1);
}

/// A role binding is on the organization, one project or one service
/// account, never both of the last: `--project` and `--service-account` are
/// refused together before anything is sent, on a create as on a list,
/// and either alone reaches the request as its full name.
#[tokio::test(flavor = "multi_thread")]
async fn a_role_binding_is_limited_to_a_project_or_a_service_account() {
    const ACCOUNT: &str = "//iam.origincloudapis.eu/organizations/o/serviceAccounts/sa-1";
    const PROJECT: &str = "//resourcemanager.origincloudapis.eu/projects/p1";
    const BINDING: &str = r#"{"name":"organizations/o/roleBindings/rb-1",
        "member":"//iam.origincloudapis.eu/users/u-1",
        "role":"roles/iam.service-account-token-creator",
        "serviceAccount":"//iam.origincloudapis.eu/organizations/o/serviceAccounts/sa-1",
        "state":"ACTIVE"}"#;
    let server = recording(|request: &Received| {
        if request.method == "GET" {
            Response::json(format!(r#"{{"roleBindings":[{BINDING}]}}"#))
        } else {
            Response::json(BINDING)
        }
    })
    .await;
    let create = [
        "create-role-binding",
        "--organization",
        "o",
        "--member",
        "//iam.origincloudapis.eu/users/u-1",
        "--role",
        "//iam.origincloudapis.eu/roles/iam.service-account-token-creator",
    ];
    let list = ["list-role-bindings", "--organization", "o"];

    for args in [&create[..], &list[..]] {
        let both = [args, &["--project", PROJECT, "--service-account", ACCOUNT]].concat();
        let output = server.run("iam", &both).await;
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{both:?}: {stderr}");
        assert!(stderr.contains("cannot be used with"), "{stderr}");
    }
    assert!(server.requests().is_empty());

    success(
        &server
            .run(
                "iam",
                &[&create[..], &["--service-account", ACCOUNT]].concat(),
            )
            .await,
    );
    let printed = success(
        &server
            .run(
                "iam",
                &[&list[..], &["--service-account", ACCOUNT]].concat(),
            )
            .await,
    );
    assert_eq!(printed["roleBindings"][0]["serviceAccount"], ACCOUNT);
    let requests = server.requests();
    let sent: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(sent["serviceAccount"], ACCOUNT);
    assert!(sent.get("project").is_none(), "{sent}");
    assert_eq!(requests[1].query_values("serviceAccount"), [ACCOUNT]);
}
