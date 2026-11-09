//! Without a login session or credentials in the environment, `origin`
//! calls the API as the service account the instance runs as, with the
//! credential of the instance's metadata service, which a local server
//! plays here, at `ORIGINCLOUD_METADATA_ENDPOINT`.

mod common;

use std::time::{Duration, Instant};

use common::{failure, origin, success};
use fake_server::{FakeServer, Received, Response};

const ZONES: &str = r#"{"zones":[{"name":"zones/z-a","displayName":"Zone A"}]}"#;

/// The metadata service, granting the session `session-1` and the
/// credential `instance-token`, and the location service, both at one
/// address.
async fn instance() -> (FakeServer, std::sync::Arc<std::sync::Mutex<Vec<Received>>>) {
    let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = std::sync::Arc::clone(&received);
    let server = FakeServer::start(move |r: Received| {
        let response = match (r.method.as_str(), r.path.as_str()) {
            ("PUT", "/latest/api/token") => Response {
                status: 200,
                headers: Vec::new(),
                body: b"session-1".to_vec(),
            },
            ("GET", "/latest/meta-data/service-account/token") => {
                if r.header("x-origincloud-metadata-token") == Some("session-1") {
                    Response::json(
                        r#"{"access_token":"instance-token","token_type":"Bearer","expires_at":"2099-01-01T00:00:00Z"}"#,
                    )
                } else {
                    Response::json("").with_status(401)
                }
            }
            (_, "/v1beta1/zones") => Response::json(ZONES),
            _ => Response::json("").with_status(404),
        };
        seen.lock().unwrap().push(r);
        async move { response }
    })
    .await;
    (server, received)
}

#[tokio::test(flavor = "multi_thread")]
async fn on_an_instance_the_commands_call_as_its_service_account() {
    let (server, received) = instance().await;
    let home = tempfile::tempdir().unwrap();
    let url = server.url();
    // A proxy that doesn't exist: the metadata service is asked directly.
    let env = [
        ("ORIGINCLOUD_METADATA_ENDPOINT", url.as_str()),
        ("HTTP_PROXY", "http://127.0.0.1:9"),
        ("http_proxy", "http://127.0.0.1:9"),
        ("NO_PROXY", "127.0.0.1"),
    ];

    let zones = success(
        &origin(
            home.path(),
            &["location", "--endpoint", &url, "list-zones"],
            &env,
        )
        .await,
    );
    assert_eq!(zones["zones"][0]["name"], "zones/z-a");
    let call = received
        .lock()
        .unwrap()
        .iter()
        .find(|r| r.path == "/v1beta1/zones")
        .cloned()
        .expect("the API was called");
    assert_eq!(call.header("authorization"), Some("Bearer instance-token"));

    // `origin id token` prints it, for other tools.
    let printed = success(&origin(home.path(), &["id", "token"], &env).await);
    assert_eq!(printed["token"], "instance-token");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_login_session_comes_before_the_metadata_service() {
    let (server, received) = instance().await;
    let home = tempfile::tempdir().unwrap();
    common::write_session(home.path());
    let url = server.url();
    let env = [("ORIGINCLOUD_METADATA_ENDPOINT", url.as_str())];

    success(
        &origin(
            home.path(),
            &["location", "--endpoint", &url, "list-zones"],
            &env,
        )
        .await,
    );
    let requests = received.lock().unwrap().clone();
    assert!(
        requests.iter().all(|r| !r.path.starts_with("/latest/")),
        "the metadata service was asked"
    );
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer stored-access")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn off_an_instance_the_command_says_how_to_log_in_quickly() {
    // A port nothing listens on: bound, then released.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let metadata = format!("http://127.0.0.1:{port}");
    let home = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let description = failure(
        &origin(
            home.path(),
            &["location", "--endpoint", "http://127.0.0.1:9", "list-zones"],
            &[("ORIGINCLOUD_METADATA_ENDPOINT", metadata.as_str())],
        )
        .await,
    );
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    assert!(description.contains("origin id login"), "{description}");
    assert!(
        description.contains("an instance that runs as a service account"),
        "{description}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_metadata_service_can_be_left_out() {
    let (server, received) = instance().await;
    let home = tempfile::tempdir().unwrap();
    let url = server.url();
    let description = failure(
        &origin(
            home.path(),
            &["location", "--endpoint", &url, "list-zones"],
            &[
                ("ORIGINCLOUD_METADATA_ENDPOINT", url.as_str()),
                ("ORIGINCLOUD_METADATA_DISABLED", "true"),
            ],
        )
        .await,
    );
    assert!(description.contains("not logged in"), "{description}");
    assert!(received.lock().unwrap().is_empty());
}
