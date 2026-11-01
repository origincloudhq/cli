//! Every method of every service of the PVS API has an `origin pvs` command,
//! checked from the API's descriptors: a method the API gains without a
//! command fails this test, and so does a command whose method is gone, or
//! an entry of the generator's table (`generator/commands.toml`) that names
//! a method or field the API doesn't have.
//!
//! The descriptors are the ones the generator checked in beside the
//! commands (`src/pvs/generated/pvs.binpb`), from the API revision
//! `API_REVISION` names; `scripts/generate.sh --check` keeps both current.

use std::collections::BTreeSet;

use heck::{ToKebabCase, ToSnakeCase};
use prost_reflect::{DescriptorPool, Kind, MethodDescriptor};
use serde::Deserialize;

const DESCRIPTORS: &[u8] = include_bytes!("../src/pvs/generated/pvs.binpb");
const GENERATED: &str = include_str!("../src/pvs/generated/commands.rs");
const TABLE: &str = include_str!("../../../generator/commands.toml");

#[derive(Deserialize)]
struct Table {
    #[serde(default)]
    exclude: Vec<Entry>,
    #[serde(default)]
    one_way: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    method: String,
    field: String,
}

fn table() -> Table {
    toml::from_str(TABLE).expect("generator/commands.toml parses")
}

fn methods() -> Vec<MethodDescriptor> {
    let pool = DescriptorPool::decode(DESCRIPTORS).expect("the checked-in descriptors decode");
    pool.services()
        .filter(|s| s.package_name().starts_with("origincloud.pvs."))
        .flat_map(|s| s.methods().collect::<Vec<_>>())
        .collect()
}

fn pvs() -> clap::Command {
    origincloud_cli::command()
        .find_subcommand("pvs")
        .expect("`origin pvs` exists")
        .clone()
}

fn has_flag(command: &clap::Command, long: &str) -> bool {
    command.get_arguments().any(|a| a.get_long() == Some(long))
}

#[test]
fn every_method_has_a_command() {
    let methods = methods();
    assert!(
        methods.len() >= 15,
        "the descriptors list only {} methods",
        methods.len()
    );
    let pvs = pvs();
    let missing: Vec<String> = methods
        .iter()
        .filter(|m| pvs.find_subcommand(m.name().to_kebab_case()).is_none())
        .map(|m| {
            format!(
                "{} (origin pvs {})",
                m.full_name(),
                m.name().to_kebab_case()
            )
        })
        .collect();
    assert!(
        missing.is_empty(),
        "these methods have no command; extend the generator so it makes one:\n  {}",
        missing.join("\n  ")
    );
}

#[test]
fn every_command_has_a_method() {
    let methods: BTreeSet<String> = methods().iter().map(|m| m.name().to_kebab_case()).collect();
    let extra: Vec<String> = pvs()
        .get_subcommands()
        .map(|c| c.get_name().to_owned())
        .filter(|name| !methods.contains(name))
        .collect();
    assert!(
        extra.is_empty(),
        "these commands have no method in the API: {extra:?}"
    );
}

/// Every entry of the table names a method of the API and a field of its
/// request, so that none silently stops applying.
#[test]
fn every_table_entry_names_a_method_and_a_field() {
    let methods = methods();
    let table = table();
    for entry in table.exclude.iter().chain(&table.one_way) {
        let method = methods
            .iter()
            .find(|m| m.name() == entry.method)
            .unwrap_or_else(|| {
                panic!(
                    "commands.toml names the method {}, which the API doesn't have",
                    entry.method
                )
            });
        let mut message = method.input();
        let mut parts = entry.field.split('.').peekable();
        while let Some(part) = parts.next() {
            let field = message.get_field_by_name(part).unwrap_or_else(|| {
                panic!(
                    "commands.toml names {} {}, which the API doesn't have",
                    entry.method, entry.field
                )
            });
            if parts.peek().is_some() {
                let Kind::Message(next) = field.kind() else {
                    panic!("{} {}: {part} isn't a message", entry.method, entry.field);
                };
                message = next;
            }
        }
    }
}

/// An excluded field has no flag; a one-way field has one, and its command
/// takes `--yes`.
#[test]
fn the_table_shapes_the_commands() {
    let pvs = pvs();
    let table = table();
    for entry in &table.exclude {
        let command = pvs.find_subcommand(entry.method.to_kebab_case()).unwrap();
        let flag = entry.field.rsplit('.').next().unwrap().to_kebab_case();
        assert!(
            !has_flag(command, &flag),
            "origin pvs {} has --{flag}, which commands.toml excludes",
            command.get_name()
        );
    }
    for entry in &table.one_way {
        let command = pvs.find_subcommand(entry.method.to_kebab_case()).unwrap();
        let flag = entry.field.rsplit('.').next().unwrap().to_kebab_case();
        assert!(
            has_flag(command, &flag),
            "no --{flag} on {}",
            command.get_name()
        );
        assert!(
            has_flag(command, "yes"),
            "no --yes on {}",
            command.get_name()
        );
    }
}

/// The text of the generated function `name`'s signature.
fn signature(name: &str) -> &'static str {
    let start = GENERATED
        .find(&format!("async fn {name}("))
        .unwrap_or_else(|| panic!("no generated function {name}"));
    let end = start + GENERATED[start..].find('{').unwrap();
    &GENERATED[start..end]
}

/// A delete, and a one-way change, reach the SDK only through a wrapper
/// that takes the client only `--yes` makes; and the SDK's method is called
/// nowhere else.
#[test]
fn deletes_and_one_way_changes_need_the_confirmed_client() {
    let table = table();
    for method in methods() {
        let snake = method.name().to_snake_case();
        let calls = GENERATED.matches(&format!(".{snake}(request)")).count();
        let one_way = table.one_way.iter().any(|o| o.method == method.name());
        if method.name().starts_with("Delete") {
            assert!(
                signature(&format!("call_{snake}")).contains("&ConfirmedClient"),
                "call_{snake} doesn't take the confirmed client"
            );
            assert_eq!(calls, 1, "{snake} is called outside its wrapper");
        } else if one_way {
            assert!(signature(&format!("call_{snake}_confirmed")).contains("&ConfirmedClient"));
            assert!(signature(&format!("call_{snake}")).contains("&ReadClient"));
            assert_eq!(calls, 2, "{snake} is called outside its wrappers");
        } else {
            assert!(signature(&format!("call_{snake}")).contains("&ReadClient"));
            assert_eq!(calls, 1, "{snake} is called outside its wrapper");
        }
    }
}

/// The hand-written commands follow the same rule: every irreversible one
/// takes `--yes`.
#[test]
fn every_hand_written_destructive_command_requires_yes() {
    let root = origincloud_cli::command();
    let mut checked = 0;
    for group in root.get_subcommands().filter(|g| g.get_name() != "pvs") {
        for command in group.get_subcommands() {
            let name = command.get_name();
            let destructive = name.starts_with("delete-")
                || name.starts_with("terminate-")
                || name == "release-floating-ip"
                || name == "revoke-service-account";
            if destructive {
                checked += 1;
                assert!(
                    has_flag(command, "yes"),
                    "origin {} {name} has no --yes",
                    group.get_name()
                );
            }
        }
    }
    assert!(checked >= 15, "only {checked} destructive commands found");
}

#[test]
fn the_command_tree_is_valid() {
    origincloud_cli::command().debug_assert();
}
