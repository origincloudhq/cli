//! What the tests of the generated commands share: the API's descriptors
//! the generator checked in, and a local server that records every request
//! and runs `origin` against it.

// Each test file uses its own part of this.
#![allow(dead_code)]

use std::path::Path;
use std::process::Output;
use std::sync::{Arc, Mutex};

use fake_server::{FakeServer, Received, Response};
use heck::ToKebabCase;
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor, MethodDescriptor, Value};

/// The descriptors of every package the commands are generated from, from
/// the API revision `API_REVISION` names; `scripts/generate.sh --check`
/// keeps them current.
pub const DESCRIPTORS: &[u8] = include_bytes!("../../src/sdk/generated/api.binpb");
pub const TABLE: &str = include_str!("../../../../generator/commands.toml");
/// origincloudhq/mcp's table at the commit `Cargo.lock` pins, as
/// `scripts/generate.sh` copied it: its `[[one_way]]` list is the one list
/// of the changes that can't be undone, which need `--yes`.
pub const MCP_TABLE: &str = include_str!("../../src/sdk/generated/mcp-tools.toml");

/// The service whose package holds the operation methods, which are
/// commands of every group whose methods answer with operations.
pub const OPERATIONS: &str = "longrunning";

pub fn pool() -> DescriptorPool {
    DescriptorPool::decode(DESCRIPTORS).expect("the checked-in descriptors decode")
}

/// The service a package names: `pvs` for `origincloud.pvs.v1beta1`.
pub fn service_of(package: &str) -> &str {
    package.split('.').nth(1).unwrap_or_default()
}

/// Every method of the API's services, by the service its package names.
pub fn methods(pool: &DescriptorPool) -> Vec<(String, MethodDescriptor)> {
    pool.services()
        .filter(|s| s.package_name().starts_with("origincloud."))
        .flat_map(|s| s.methods().collect::<Vec<_>>())
        .map(|m| (service_of(m.parent_service().package_name()).to_owned(), m))
        .collect()
}

/// The command groups generated from the API: every service but the
/// operations one, in order.
pub fn groups(pool: &DescriptorPool) -> Vec<String> {
    let mut groups: Vec<String> = methods(pool)
        .into_iter()
        .map(|(service, _)| service)
        .filter(|s| s != OPERATIONS)
        .collect();
    groups.sort();
    groups.dedup();
    groups
}

/// Whether `m` answers with an operation it starts.
pub fn starts_an_operation(pool: &DescriptorPool, m: &MethodDescriptor) -> bool {
    extension(pool, &m.options(), "google.longrunning.operation_info").is_some()
}

/// The groups with the operation methods' commands: those with a method
/// answering with an operation.
pub fn operation_groups(pool: &DescriptorPool) -> Vec<String> {
    let mut groups: Vec<String> = methods(pool)
        .into_iter()
        .filter(|(service, m)| service != OPERATIONS && starts_an_operation(pool, m))
        .map(|(service, _)| service)
        .collect();
    groups.sort();
    groups.dedup();
    groups
}

pub fn extension(
    pool: &DescriptorPool,
    options: &DynamicMessage,
    name: &str,
) -> Option<DynamicMessage> {
    let ext = pool.get_extension_by_name(name)?;
    if !options.has_extension(&ext) {
        return None;
    }
    match options.get_extension(&ext).into_owned() {
        Value::Message(m) => Some(m),
        _ => None,
    }
}

pub fn pattern_of_type(pool: &DescriptorPool, resource_type: &str) -> String {
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

pub fn pattern_of_message(pool: &DescriptorPool, message: &MessageDescriptor) -> String {
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
pub fn whole_name(pattern: &str) -> (String, String) {
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

/// A login session valid until 2099, in `dir` as `XDG_DATA_HOME`.
pub fn write_session(dir: &Path) {
    let path = dir.join("origincloud").join("session.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        r#"{"version":1,"access_token":"stored-access","expires":"2099-01-01T00:00:00Z","refresh_token":"stored-refresh","refresh_expires":"2099-01-01T00:00:00Z","token_type":"Bearer"}"#,
    )
    .unwrap();
}

pub struct Recording {
    pub server: FakeServer,
    received: Arc<Mutex<Vec<Received>>>,
    pub home: tempfile::TempDir,
}

/// A server answering every call with `answer`, recording each request,
/// and a home directory with a login session.
pub async fn recording(
    answer: impl Fn(&Received) -> Response + Send + Sync + 'static,
) -> Recording {
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

impl Recording {
    /// Runs `origin {group}` with `args` against the server, with `env` set.
    pub async fn run_with(&self, group: &str, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_origin"));
        command
            .arg(group)
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

    pub async fn run(&self, group: &str, args: &[&str]) -> Output {
        self.run_with(group, args, &[]).await
    }

    pub async fn pvs_with(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        self.run_with("pvs", args, env).await
    }

    pub async fn pvs(&self, args: &[&str]) -> Output {
        self.run("pvs", args).await
    }

    /// Runs `origin pvs` with `args`, the zone lookup of a list given no
    /// `--zone` sent to the server too, through the location group's
    /// variable.
    pub async fn pvs_zonal(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let url = self.server.url();
        let mut env = env.to_vec();
        env.push(("ORIGINCLOUD_LOCATION_ENDPOINT", &url));
        self.run_with("pvs", args, &env).await
    }

    pub fn requests(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }
}

/// Runs `origin` with `args` and only `env` in its environment, with a
/// login session in `home`.
pub async fn origin(home: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_origin"));
    command
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("XDG_DATA_HOME", home)
        .env("XDG_CONFIG_HOME", home);
    for (name, value) in env {
        command.env(name, value);
    }
    tokio::task::spawn_blocking(move || command.output().expect("origin runs"))
        .await
        .unwrap()
}

pub fn stdout(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "not JSON: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// A command that succeeded, and what it printed.
pub fn success(output: &Output) -> serde_json::Value {
    assert_eq!(output.status.code(), Some(0), "{}", stdout(output));
    stdout(output)
}

/// A refused command: exit status 1 and the failure envelope.
pub fn failure(output: &Output) -> String {
    assert_eq!(output.status.code(), Some(1), "{}", stdout(output));
    let printed = stdout(output);
    assert_eq!(printed["code"], "failure", "{printed}");
    printed["description"].as_str().unwrap().to_owned()
}
