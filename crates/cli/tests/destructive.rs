//! `origin pvs` against a local server that records every request: the
//! guard of every delete and one-way change, driven from the API's
//! descriptors, the credentials, the checks on names and endpoints, and the
//! lists.

use std::path::Path;
use std::process::Output;
use std::sync::{Arc, Mutex};

use fake_server::{FakeServer, Received, Response};
use heck::ToKebabCase;
use prost_reflect::{DescriptorPool, MessageDescriptor, Value};
use serde::Deserialize;

const DESCRIPTORS: &[u8] = include_bytes!("../src/pvs/generated/pvs.binpb");
const TABLE: &str = include_str!("../../../generator/commands.toml");

/// A login session valid until 2099, in `dir` as `XDG_DATA_HOME`.
fn write_session(dir: &Path) {
    let path = dir.join("origincloud").join("session.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        r#"{"version":1,"access_token":"stored-access","expires":"2099-01-01T00:00:00Z","refresh_token":"stored-refresh","refresh_expires":"2099-01-01T00:00:00Z","token_type":"Bearer"}"#,
    )
    .unwrap();
}

type Answer = fn(&Received) -> Response;

struct Recording {
    server: FakeServer,
    received: Arc<Mutex<Vec<Received>>>,
    home: tempfile::TempDir,
}

/// A server answering every call with `answer`, recording each request,
/// and a home directory with a login session.
async fn recording(answer: Answer) -> Recording {
    let received = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&received);
    let server = FakeServer::start(move |request| {
        let response = answer(&request);
        seen.lock().unwrap().push(request);
        async move { response }
    })
    .await;
    let home = tempfile::tempdir().unwrap();
    write_session(home.path());
    Recording {
        server,
        received,
        home,
    }
}

fn answering(body: &'static str) -> Answer {
    // A closure can't be a `fn` pointer when it captures, so the bodies the
    // tests use are matched here.
    match body {
        DELETED => |_| Response::json(DELETED),
        UPDATED => |_| Response::json(UPDATED),
        VOLUME => |_| Response::json(VOLUME),
        _ => |_| Response::json("{}"),
    }
}

impl Recording {
    /// Runs `origin pvs` with `args` against the server, with `env` set.
    async fn pvs_with(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_origin"));
        command
            .arg("pvs")
            .args(["--endpoint", &self.server.url()])
            .args(args)
            .env_clear()
            .env("HOME", self.home.path())
            .env("XDG_DATA_HOME", self.home.path())
            .env("XDG_CONFIG_HOME", self.home.path());
        for (name, value) in env {
            command.env(name, value);
        }
        tokio::task::spawn_blocking(move || command.output().expect("origin runs"))
            .await
            .unwrap()
    }

    async fn pvs(&self, args: &[&str]) -> Output {
        self.pvs_with(args, &[]).await
    }

    fn requests(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }
}

const DELETED: &str = r#"{"name":"projects/p/zones/z/operations/op-1","done":true,"response":{"@type":"type.googleapis.com/google.protobuf.Empty"}}"#;
const UPDATED: &str = r#"{"name":"projects/p/zones/z/operations/op-2","done":true,"response":{"@type":"type.googleapis.com/origincloud.pvs.v1beta1.Volume","name":"projects/p/zones/z/volumes/v","sizeGib":20}}"#;
const VOLUME: &str = r#"{"name":"projects/p/zones/z/volumes/v","sizeGib":8}"#;

fn stdout(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "not JSON: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// A refused command: exit status 1 and the failure envelope.
fn failure(output: &Output) -> String {
    assert_eq!(output.status.code(), Some(1), "{}", stdout(output));
    let printed = stdout(output);
    assert_eq!(printed["code"], "failure", "{printed}");
    printed["description"].as_str().unwrap().to_owned()
}

// The descriptors: what a command's resource flag is and what a whole name
// of its resource looks like.

fn pool() -> DescriptorPool {
    DescriptorPool::decode(DESCRIPTORS).expect("the checked-in descriptors decode")
}

fn extension(
    pool: &DescriptorPool,
    options: &prost_reflect::DynamicMessage,
    name: &str,
) -> Option<prost_reflect::DynamicMessage> {
    let ext = pool.get_extension_by_name(name)?;
    if !options.has_extension(&ext) {
        return None;
    }
    match options.get_extension(&ext).into_owned() {
        Value::Message(m) => Some(m),
        _ => None,
    }
}

fn pattern_of_type(pool: &DescriptorPool, resource_type: &str) -> String {
    pool.all_messages()
        .find_map(|m| {
            let r = extension(pool, &m.options(), "google.api.resource")?;
            (r.get_field_by_name("type")?.as_str()? == resource_type).then(|| {
                match r.get_field_by_name("pattern").unwrap().into_owned() {
                    Value::List(p) => p[0].as_str().unwrap().to_owned(),
                    _ => unreachable!(),
                }
            })
        })
        .unwrap_or_else(|| panic!("no resource {resource_type}"))
}

fn pattern_of_message(pool: &DescriptorPool, message: &MessageDescriptor) -> String {
    let r = extension(pool, &message.options(), "google.api.resource").expect("a resource");
    let t = r
        .get_field_by_name("type")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    pattern_of_type(pool, &t)
}

/// The resource flag of `pattern` and a whole name of it.
fn whole_name(pattern: &str) -> (String, String) {
    let mut leaf = String::new();
    let name: Vec<String> = pattern
        .split('/')
        .map(
            |s| match s.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                Some(var) => {
                    leaf = var.to_owned();
                    format!("{}-1", var.replace('_', "-"))
                }
                None => s.to_owned(),
            },
        )
        .collect();
    (leaf.to_kebab_case(), name.join("/"))
}

#[derive(Deserialize)]
struct Table {
    #[serde(default)]
    one_way: Vec<OneWay>,
}

#[derive(Deserialize)]
struct OneWay {
    method: String,
    field: String,
}

/// Every delete without `--yes`, its resource given by its whole name, is
/// refused before any request is sent, and says how to go on.
#[tokio::test(flavor = "multi_thread")]
async fn every_delete_requires_yes() {
    let pool = pool();
    let server = recording(answering(DELETED)).await;
    let mut checked = 0;
    for method in pool
        .services()
        .filter(|s| s.package_name().starts_with("origincloud.pvs."))
        .flat_map(|s| s.methods().collect::<Vec<_>>())
        .filter(|m| m.name().starts_with("Delete"))
    {
        let field = method
            .input()
            .get_field_by_name("name")
            .expect("a delete names its resource");
        let reference =
            extension(&pool, &field.options(), "google.api.resource_reference").unwrap();
        let t = reference
            .get_field_by_name("type")
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let (flag, name) = whole_name(&pattern_of_type(&pool, &t));
        let command = method.name().to_kebab_case();

        let output = server.pvs(&[&command, &format!("--{flag}"), &name]).await;
        let description = failure(&output);
        assert!(
            description.starts_with("refusing to delete "),
            "{command}: {description}"
        );
        assert!(description.contains(&name), "{command}: {description}");
        assert!(
            description.ends_with(&format!(" {command} --{flag} {name} --yes")),
            "{command}: {description}"
        );
        checked += 1;
    }
    assert!(checked >= 2);
    assert!(
        server.requests().is_empty(),
        "a refused delete sent {:?}",
        server.requests()
    );
}

/// Every one-way change the generator's table names, without `--yes`, is
/// refused before any request, naming the resource and the change.
#[tokio::test(flavor = "multi_thread")]
async fn every_one_way_change_requires_yes() {
    let pool = pool();
    let table: Table = toml::from_str(TABLE).unwrap();
    let server = recording(answering(UPDATED)).await;
    assert!(!table.one_way.is_empty());
    for entry in &table.one_way {
        let method = pool
            .services()
            .flat_map(|s| s.methods().collect::<Vec<_>>())
            .find(|m| m.name() == entry.method)
            .unwrap();
        let (body, field) = entry
            .field
            .split_once('.')
            .expect("a field of the resource");
        let prost_reflect::Kind::Message(resource) =
            method.input().get_field_by_name(body).unwrap().kind()
        else {
            panic!("{body} isn't a message");
        };
        let (flag, name) = whole_name(&pattern_of_message(&pool, &resource));
        let value = match resource.get_field_by_name(field).unwrap().kind() {
            prost_reflect::Kind::Bool => "true",
            prost_reflect::Kind::String => "x",
            _ => "20",
        };
        let change = format!("--{} {value}", field.to_kebab_case());
        let command = method.name().to_kebab_case();
        let flag_arg = format!("--{flag}");
        let output = server
            .pvs(&[
                &command,
                &flag_arg,
                &name,
                &format!("--{}", field.to_kebab_case()),
                value,
            ])
            .await;
        let description = failure(&output);
        assert!(description.starts_with("refusing to "), "{description}");
        assert!(
            description.contains(&format!("{name} ({change})")),
            "{description}"
        );
        assert!(description.ends_with("--yes"), "{description}");
    }
    assert!(server.requests().is_empty(), "{:?}", server.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_delete_with_yes_is_sent_with_the_login_session() {
    let server = recording(answering(DELETED)).await;
    let output = server
        .pvs(&[
            "delete-volume",
            "--volume",
            "projects/p/zones/z/volumes/v",
            "--yes",
        ])
        .await;

    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    assert_eq!(stdout(&output)["code"], "success");
    let requests = server.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(requests[0].path, "/v1beta1/projects/p/zones/z/volumes/v");
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer stored-access")
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("instead of the service's own host"),
        "the endpoint override is said on stderr"
    );
}

/// An update names in its mask exactly the fields its flags set; `--no-tags`
/// empties the tags by name, and needs no `--yes`; growing does.
#[tokio::test(flavor = "multi_thread")]
async fn an_update_sends_the_fields_its_flags_name() {
    let server = recording(answering(UPDATED)).await;
    let volume = ["--project", "p", "--zone", "z", "--volume", "v"];
    let output = server
        .pvs(
            &[
                &["update-volume"][..],
                &volume,
                &["--size-gib", "20", "--yes"],
            ]
            .concat(),
        )
        .await;
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    assert_eq!(stdout(&output)["sizeGib"], 20);

    let output = server
        .pvs(&[&["update-volume"][..], &volume, &["--no-tags"]].concat())
        .await;
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));

    let requests = server.requests();
    assert_eq!(requests.len(), 2, "{requests:?}");
    assert_eq!(requests[0].method, "PATCH");
    assert_eq!(requests[0].path, "/v1beta1/projects/p/zones/z/volumes/v");
    assert_eq!(requests[0].query_values("updateMask"), ["sizeGib"]);
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["sizeGib"], 20);
    assert_eq!(requests[1].query_values("updateMask"), ["tags"]);

    // Neither a field nor `--no-tags`, or both `--tag` and `--no-tags`:
    // refused by the parser, nothing sent.
    let output = server
        .pvs(&[&["update-volume"][..], &volume].concat())
        .await;
    assert_eq!(output.status.code(), Some(2));
    let output = server
        .pvs(
            &[
                &["update-volume"][..],
                &volume,
                &["--tag", "a=b", "--no-tags"],
            ]
            .concat(),
        )
        .await;
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(server.requests().len(), 2);
}

/// The credentials in the environment come first, as in every SDK; without
/// them, the login session; without either, a message saying how to log in.
#[tokio::test(flavor = "multi_thread")]
async fn credentials_come_from_the_environment_then_the_session() {
    let server = recording(answering(VOLUME)).await;
    let get = ["get-volume", "--volume", "projects/p/zones/z/volumes/v"];

    let output = server
        .pvs_with(&get, &[("ORIGINCLOUD_ACCESS_TOKEN", "from-environment")])
        .await;
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    let output = server.pvs(&get).await;
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    let tokens: Vec<_> = server
        .requests()
        .iter()
        .map(|r| r.header("authorization").unwrap_or_default().to_owned())
        .collect();
    assert_eq!(tokens, ["Bearer from-environment", "Bearer stored-access"]);

    std::fs::remove_file(server.home.path().join("origincloud").join("session.json")).unwrap();
    let description = failure(&server.pvs(&get).await);
    assert!(description.contains("origin id login"), "{description}");
    assert!(
        description.contains("ORIGINCLOUD_ACCESS_TOKEN"),
        "{description}"
    );
    assert!(
        description.contains("ORIGINCLOUD_CLIENT_ID"),
        "{description}"
    );
    assert_eq!(server.requests().len(), 2, "no request without credentials");
}

/// A part of a name that could step out of its place in the path is
/// refused before any request.
#[tokio::test(flavor = "multi_thread")]
async fn a_name_part_outside_the_id_alphabet_is_refused() {
    let server = recording(answering(VOLUME)).await;
    for args in [
        &[
            "get-volume",
            "--project",
            "p",
            "--zone",
            "z",
            "--volume",
            "..",
        ][..],
        &[
            "get-volume",
            "--project",
            "p",
            "--zone",
            "z",
            "--volume",
            ".",
        ],
        &[
            "get-volume",
            "--project",
            "a/b",
            "--zone",
            "z",
            "--volume",
            "v",
        ],
        &["get-volume", "--volume", "projects/p/zones/../volumes/v"],
    ] {
        let description = failure(&server.pvs(args).await);
        assert!(
            description.contains("isn't an id") || description.contains("isn't a name"),
            "{args:?}: {description}"
        );
    }
    assert!(server.requests().is_empty(), "{:?}", server.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_endpoint_is_https_or_this_machine_and_only_for_pvs() {
    let server = recording(answering(VOLUME)).await;
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_origin"));
    let output = command
        .args([
            "pvs",
            "--endpoint",
            "http://192.0.2.1:9",
            "get-volume",
            "--volume",
            "projects/p/zones/z/volumes/v",
        ])
        .env_clear()
        .env("HOME", server.home.path())
        .output()
        .unwrap();
    let description = failure(&output);
    assert!(
        description.contains("http:// is only for this machine"),
        "{description}"
    );

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_origin"))
        .args([
            "--endpoint",
            "https://example.com",
            "admin",
            "list-organisations",
        ])
        .env_clear()
        .env("HOME", server.home.path())
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(2),
        "--endpoint is an option of pvs only"
    );
}

const ZONES: &str = r#"{"zones":[
    {"name":"zones/z-a","region":"regions/r","services":["pvs"]},
    {"name":"zones/z-b","region":"regions/r"},
    {"name":"zones/z-c","region":"regions/r","services":["other","pvs"]}]}"#;

/// The location service and PVS, as the server answers them: zone `z-a`
/// holds `v-a`, `z-c` holds `v-c` on two pages, `z-fail` fails, and the
/// project `empty` holds nothing.
fn zonal(request: &Received) -> Response {
    let page = request.query_values("pageToken");
    match request.path.as_str() {
        "/v1beta1/zones" => Response::json(ZONES),
        "/v1beta1/projects/p/zones/z-a/volumes" => {
            Response::json(r#"{"volumes":[{"name":"projects/p/zones/z-a/volumes/v-a"}]}"#)
        }
        "/v1beta1/projects/p/zones/z-c/volumes" if page.is_empty() => Response::json(
            r#"{"volumes":[{"name":"projects/p/zones/z-c/volumes/v-c1"}],"nextPageToken":"next"}"#,
        ),
        "/v1beta1/projects/p/zones/z-c/volumes" => {
            Response::json(r#"{"volumes":[{"name":"projects/p/zones/z-c/volumes/v-c2"}]}"#)
        }
        path if path.starts_with("/v1beta1/projects/empty/") => Response::json("{}"),
        _ => Response::json(r#"{"error":{"code":500,"status":"INTERNAL","message":"broken"}}"#)
            .with_status(500),
    }
}

fn names(output: &Output) -> Vec<String> {
    stdout(output)["volumes"]
        .as_array()
        .unwrap_or_else(|| panic!("{}", stdout(output)))
        .iter()
        .map(|v| v["name"].as_str().unwrap().to_owned())
        .collect()
}

/// Without `--zone`, a list covers every zone offering the service, in
/// catalog order, all pages of each; the profile's zone isn't used.
#[tokio::test(flavor = "multi_thread")]
async fn a_list_without_a_zone_lists_every_zone_of_the_service() {
    let server = recording(zonal).await;
    let output = server
        .pvs_with(
            &["list-volumes", "--project", "p"],
            &[("ORIGINCLOUD_ZONE", "z-b")],
        )
        .await;
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    assert_eq!(
        names(&output),
        [
            "projects/p/zones/z-a/volumes/v-a",
            "projects/p/zones/z-c/volumes/v-c1",
            "projects/p/zones/z-c/volumes/v-c2"
        ]
    );
    let paths: Vec<String> = server.requests().iter().map(|r| r.path.clone()).collect();
    assert_eq!(paths[0], "/v1beta1/zones");
    assert!(!paths.iter().any(|p| p.contains("z-b")), "{paths:?}");

    // `--zone` lists that zone alone.
    let output = server
        .pvs(&["list-volumes", "--project", "p", "--zone", "z-a"])
        .await;
    assert_eq!(names(&output), ["projects/p/zones/z-a/volumes/v-a"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_list_fails_whole_when_a_zone_fails() {
    let server = recording(zonal).await;
    let description = failure(
        &server
            .pvs(&["list-volumes", "--project", "p", "--zone", "z-fail"])
            .await,
    );
    assert!(description.contains("broken"), "{description}");
}

#[tokio::test(flavor = "multi_thread")]
async fn one_page_needs_a_zone() {
    let server = recording(zonal).await;
    let description = failure(
        &server
            .pvs(&["list-volumes", "--project", "p", "--page-size", "1"])
            .await,
    );
    assert!(description.contains("need --zone"), "{description}");
    assert!(server.requests().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_list_prints_an_empty_array() {
    let server = recording(zonal).await;
    for args in [
        &["list-volumes", "--project", "empty", "--zone", "z-a"][..],
        &["list-volumes", "--project", "empty"],
        &[
            "list-volumes",
            "--project",
            "empty",
            "--zone",
            "z-a",
            "--page-size",
            "5",
        ],
    ] {
        let output = server.pvs(args).await;
        assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
        assert_eq!(
            stdout(&output),
            serde_json::json!({ "volumes": [] }),
            "{args:?}"
        );
    }
}
