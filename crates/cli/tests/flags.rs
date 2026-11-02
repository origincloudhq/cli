//! Which flags the generated commands require, checked in clap's own
//! command tree, so that `--help` and the usage error show what a command
//! line needs before anything runs: a boolean the API doesn't require is
//! optional, and a part of a parent's name that no other flag can give is
//! required.

mod common;

use clap::{Arg, Command};
use common::{methods, origin, pool};
use heck::ToKebabCase;
use prost_reflect::{DescriptorPool, FieldDescriptor, Kind, Value};

/// `google.api.field_behavior`'s `REQUIRED` and `OUTPUT_ONLY`.
const REQUIRED: i32 = 2;
const OUTPUT_ONLY: i32 = 3;

fn behaviors(pool: &DescriptorPool, field: &FieldDescriptor) -> Vec<i32> {
    let ext = pool
        .get_extension_by_name("google.api.field_behavior")
        .expect("the API uses field_behavior");
    let options = field.options();
    if !options.has_extension(&ext) {
        return Vec::new();
    }
    match options.get_extension(&ext).as_ref() {
        Value::List(values) => values.iter().filter_map(Value::as_enum_number).collect(),
        _ => Vec::new(),
    }
}

fn subcommand(group: &str, name: &str) -> Command {
    origincloud_cli::command()
        .find_subcommand(group)
        .and_then(|g| g.find_subcommand(name))
        .unwrap_or_else(|| panic!("`origin {group} {name}` doesn't exist"))
        .clone()
}

fn flag<'a>(command: &'a Command, long: &str) -> Option<&'a Arg> {
    command.get_arguments().find(|a| a.get_long() == Some(long))
}

/// A boolean field the API doesn't require, of a request or of the
/// resource it carries, is an optional flag, in a create as in any other
/// command: left out, it is `false`, as the API reads a missing one.
#[test]
fn a_boolean_the_api_doesnt_require_is_optional() {
    let pool = pool();
    let root = origincloud_cli::command();
    let mut checked = 0;
    for (service, method) in methods(&pool) {
        let Some(command) = root
            .find_subcommand(&service)
            .and_then(|g| g.find_subcommand(method.name().to_kebab_case()))
        else {
            continue;
        };
        let request = method.input();
        let bodies: Vec<FieldDescriptor> = request
            .fields()
            .filter_map(|f| match f.kind() {
                Kind::Message(m) if !f.is_list() && !f.is_map() => {
                    Some(m.fields().collect::<Vec<_>>())
                }
                _ => None,
            })
            .flatten()
            .collect();
        for field in request.fields().chain(bodies) {
            let b = behaviors(&pool, &field);
            if field.kind() != Kind::Bool
                || field.is_list()
                || b.contains(&REQUIRED)
                || b.contains(&OUTPUT_ONLY)
            {
                continue;
            }
            let long = field.name().to_kebab_case();
            if let Some(arg) = flag(command, &long) {
                assert!(
                    !arg.is_required_set(),
                    "origin {service} {} requires --{long}, which the API doesn't",
                    command.get_name()
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "no optional boolean was checked");
    for (group, name, long) in [
        ("network", "create-subnet", "auto-assign-public-ip"),
        ("network", "create-security-group-rule", "same-group"),
    ] {
        let command = subcommand(group, name);
        assert!(
            !flag(&command, long).unwrap().is_required_set(),
            "{name} --{long}"
        );
    }
}

/// A part of the parent's name that no other flag gives, such as the
/// instance of a volume attachment created or listed, is required, and in
/// the usage line; one that a flag given a whole name can give, as
/// `--volume-attachment projects/.../instances/i-1/volumeAttachments/va-1`
/// gives the instance, is optional, and says so.
#[test]
fn a_parent_part_nothing_else_gives_is_required() {
    for (group, name, long) in [
        ("compute", "create-volume-attachment", "instance"),
        ("compute", "list-volume-attachments", "instance"),
        ("network", "create-subnet", "vpc"),
        ("network", "create-security-group-rule", "security-group"),
    ] {
        let mut command = subcommand(group, name);
        assert!(
            flag(&command, long).unwrap().is_required_set(),
            "{name} --{long}"
        );
        let usage = command.render_usage().to_string();
        assert!(usage.contains(&format!("--{long} <")), "{name}: {usage}");
    }
    for (group, name, long, whole) in [
        (
            "compute",
            "get-volume-attachment",
            "instance",
            "volume-attachment",
        ),
        (
            "compute",
            "delete-volume-attachment",
            "instance",
            "volume-attachment",
        ),
    ] {
        let command = subcommand(group, name);
        let arg = flag(&command, long).unwrap();
        assert!(!arg.is_required_set(), "{name} --{long}");
        let help = arg.get_help().unwrap().to_string();
        assert!(help.contains(&format!("`--{whole}`")), "{name}: {help}");
    }
    // The defaulted parts stay optional.
    let command = subcommand("compute", "create-volume-attachment");
    for long in ["project", "zone"] {
        assert!(!flag(&command, long).unwrap().is_required_set(), "--{long}");
    }
}

/// Without a required parent part, the command line is refused as clap
/// refuses any other, exit status 2, naming the flag, before anything is
/// sent.
#[tokio::test(flavor = "multi_thread")]
async fn a_missing_parent_part_is_a_usage_error() {
    let home = tempfile::tempdir().unwrap();
    let output = origin(
        home.path(),
        &[
            "compute",
            "--endpoint",
            "http://127.0.0.1:9",
            "list-volume-attachments",
            "--project",
            "p",
            "--zone",
            "z",
        ],
        &[],
    )
    .await;
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--instance <INSTANCE>"), "{stderr}");
}
