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

/// GitHub Actions' token endpoint, handing out `job-token-<n>` for the
/// STS's audience, the STS, exchanging a job token for `ci-token`, or
/// refusing it when `refuse`, and the location service, all at one address.
async fn ci_job(refuse: bool) -> (FakeServer, std::sync::Arc<std::sync::Mutex<Vec<Received>>>) {
    let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = std::sync::Arc::clone(&received);
    let issued = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let server = FakeServer::start(move |r: Received| {
        let response = match (r.method.as_str(), r.path.as_str()) {
            ("GET", "/idtoken") => {
                let n = issued.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                if r.header("authorization") == Some("bearer github-request-token")
                    && r.query_values("audience") == ["https://sts.origincloudapis.eu"]
                {
                    Response::json(format!(r#"{{"value":"job-token-{n}"}}"#))
                } else {
                    Response::json("").with_status(403)
                }
            }
            ("POST", "/v1/token") if refuse => {
                Response::json(r#"{"error":"invalid_grant"}"#).with_status(400)
            }
            ("POST", "/v1/token") => Response::json(
                r#"{"access_token":"ci-token","issued_token_type":"urn:ietf:params:oauth:token-type:access_token","token_type":"Bearer","expires_in":900}"#,
            ),
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
async fn in_a_github_actions_job_the_commands_call_as_the_service_account_it_is_trusted_as() {
    let (server, received) = ci_job(false).await;
    let home = tempfile::tempdir().unwrap();
    // A login session too: the job's token comes first.
    common::write_session(home.path());
    let url = server.url();
    let request_url = format!("{url}/idtoken?api-version=2.0");
    let env = [
        ("ACTIONS_ID_TOKEN_REQUEST_URL", request_url.as_str()),
        ("ACTIONS_ID_TOKEN_REQUEST_TOKEN", "github-request-token"),
        ("ORIGINCLOUD_STS_ENDPOINT", url.as_str()),
        ("ORIGINCLOUD_METADATA_DISABLED", "true"),
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
    let requests = received.lock().unwrap().clone();
    let exchange = requests
        .iter()
        .find(|r| r.path == "/v1/token")
        .expect("the STS was asked");
    let form: Vec<(String, String)> = url::form_urlencoded::parse(exchange.body.as_ref())
        .into_owned()
        .collect();
    assert!(
        form.contains(&("subject_token".into(), "job-token-1".into())),
        "{form:?}"
    );
    assert!(
        form.contains(&(
            "grant_type".into(),
            "urn:ietf:params:oauth:grant-type:token-exchange".into()
        )),
        "{form:?}"
    );
    let call = requests
        .iter()
        .find(|r| r.path == "/v1beta1/zones")
        .expect("the API was called");
    assert_eq!(call.header("authorization"), Some("Bearer ci-token"));

    // `origin id token` prints it, for other tools.
    let printed = success(&origin(home.path(), &["id", "token"], &env).await);
    assert_eq!(printed["token"], "ci-token");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_ci_job_s_token_comes_from_the_file_the_environment_names() {
    let (server, received) = ci_job(false).await;
    let home = tempfile::tempdir().unwrap();
    let file = home.path().join("oidc-token");
    std::fs::write(&file, "file-token\n").unwrap();
    let url = server.url();
    let request_url = format!("{url}/idtoken?api-version=2.0");
    let printed = success(
        &origin(
            home.path(),
            &["id", "token"],
            &[
                ("ORIGINCLOUD_OIDC_TOKEN_FILE", file.to_str().unwrap()),
                // GitHub's too: the file comes first.
                ("ACTIONS_ID_TOKEN_REQUEST_URL", request_url.as_str()),
                ("ACTIONS_ID_TOKEN_REQUEST_TOKEN", "github-request-token"),
                ("ORIGINCLOUD_STS_ENDPOINT", url.as_str()),
                ("ORIGINCLOUD_METADATA_DISABLED", "true"),
            ],
        )
        .await,
    );
    assert_eq!(printed["token"], "ci-token");
    let requests = received.lock().unwrap().clone();
    assert!(
        requests.iter().all(|r| r.path != "/idtoken"),
        "GitHub was asked"
    );
    let exchange = requests.iter().find(|r| r.path == "/v1/token").unwrap();
    assert!(exchange.text().contains("subject_token=file-token"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_exchange_says_what_to_check_not_that_nobody_is_logged_in() {
    let (server, _) = ci_job(true).await;
    let home = tempfile::tempdir().unwrap();
    let url = server.url();
    let request_url = format!("{url}/idtoken?api-version=2.0");
    let description = failure(
        &origin(
            home.path(),
            &["location", "--endpoint", &url, "list-zones"],
            &[
                ("ACTIONS_ID_TOKEN_REQUEST_URL", request_url.as_str()),
                ("ACTIONS_ID_TOKEN_REQUEST_TOKEN", "github-request-token"),
                ("ORIGINCLOUD_STS_ENDPOINT", url.as_str()),
                ("ORIGINCLOUD_METADATA_DISABLED", "true"),
            ],
        )
        .await,
    );
    assert!(description.contains("invalid_grant"), "{description}");
    assert!(description.contains("conditions"), "{description}");
    assert!(!description.contains("job-token"), "{description}");
    assert!(
        !description.contains("github-request-token"),
        "{description}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_ci_job_s_token_can_be_left_out() {
    let (server, received) = ci_job(false).await;
    let home = tempfile::tempdir().unwrap();
    let url = server.url();
    let request_url = format!("{url}/idtoken?api-version=2.0");
    let description = failure(
        &origin(
            home.path(),
            &["location", "--endpoint", &url, "list-zones"],
            &[
                ("ACTIONS_ID_TOKEN_REQUEST_URL", request_url.as_str()),
                ("ACTIONS_ID_TOKEN_REQUEST_TOKEN", "github-request-token"),
                ("ORIGINCLOUD_STS_ENDPOINT", url.as_str()),
                ("ORIGINCLOUD_OIDC_DISABLED", "true"),
                ("ORIGINCLOUD_METADATA_DISABLED", "true"),
            ],
        )
        .await,
    );
    assert!(description.contains("not logged in"), "{description}");
    assert!(received.lock().unwrap().is_empty());
}
