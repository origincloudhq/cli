//! The CLI covers the whole API: every method of every service has a
//! command, `origin {service} {method}` in kebab case, unless the
//! generator's table (`generator/commands.toml`) leaves it out with a
//! reason; the operation methods (`origincloud.longrunning`) are commands of
//! every group whose methods answer with operations. A method the API gains
//! without a command fails this test, and so does a command whose method is
//! gone, or an entry of the table that names a service, method or field the
//! API doesn't have.
//!
//! The descriptors are the ones the generator checked in beside the
//! commands (`src/sdk/generated/api.binpb`), from the API revision
//! `API_REVISION` names; `scripts/generate.sh --check` keeps both current.

mod common;

use std::collections::BTreeSet;

use common::{groups, methods, operation_groups, pool, OPERATIONS, TABLE};
use heck::{ToKebabCase, ToSnakeCase};
use prost_reflect::Kind;
use serde::Deserialize;

#[derive(Deserialize)]
struct Table {
    #[serde(default)]
    service: Vec<Group>,
    #[serde(default)]
    no_command: Vec<NoCommand>,
    #[serde(default)]
    exclude: Vec<Entry>,
    #[serde(default)]
    one_way: Vec<Entry>,
}

#[derive(Deserialize)]
struct Group {
    name: String,
}

#[derive(Deserialize)]
struct NoCommand {
    service: String,
    method: String,
    why: String,
}

#[derive(Deserialize)]
struct Entry {
    service: String,
    method: String,
    field: String,
}

fn table() -> Table {
    toml::from_str(TABLE).expect("generator/commands.toml parses")
}

fn group(name: &str) -> clap::Command {
    origincloud_cli::command()
        .find_subcommand(name)
        .unwrap_or_else(|| panic!("`origin {name}` doesn't exist"))
        .clone()
}

fn has_flag(command: &clap::Command, long: &str) -> bool {
    command.get_arguments().any(|a| a.get_long() == Some(long))
}

/// One method of the API and the commands that offer it: one, or, for an
/// operation method, one in each group answering with operations.
struct Rpc {
    service: String,
    full_name: String,
    method: String,
    commands: Vec<(String, String)>,
}

fn rpcs() -> Vec<Rpc> {
    let pool = pool();
    let operation_groups = operation_groups(&pool);
    methods(&pool)
        .into_iter()
        .map(|(service, m)| {
            let command = m.name().to_kebab_case();
            let commands = if service == OPERATIONS {
                operation_groups
                    .iter()
                    .map(|g| (g.clone(), command.clone()))
                    .collect()
            } else {
                vec![(service.clone(), command)]
            };
            Rpc {
                service,
                full_name: m.full_name().to_owned(),
                method: m.name().to_owned(),
                commands,
            }
        })
        .collect()
}

/// The parity guarantee: every method of the API has its command, or the
/// table says why it has none.
#[test]
fn every_method_has_a_command_or_a_reason() {
    let rpcs = rpcs();
    let table = table();
    // PVS's 15, the location service's 4 and the 2 operation methods, at
    // least: the descriptors aren't silently empty.
    assert!(
        rpcs.len() >= 21,
        "the descriptors list only {} methods",
        rpcs.len()
    );
    let mut missing = Vec::new();
    let mut stale = Vec::new();
    for rpc in &rpcs {
        let reasoned = table
            .no_command
            .iter()
            .any(|n| n.service == rpc.service && n.method == rpc.method);
        let present: Vec<bool> = rpc
            .commands
            .iter()
            .map(|(g, c)| {
                origincloud_cli::command()
                    .find_subcommand(g)
                    .is_some_and(|g| g.find_subcommand(c).is_some())
            })
            .collect();
        if reasoned {
            if present.iter().any(|p| *p) {
                stale.push(rpc.full_name.clone());
            }
        } else if rpc.commands.is_empty() {
            missing.push(format!(
                "{} (no group answers with operations to carry it)",
                rpc.full_name
            ));
        } else {
            for ((g, c), present) in rpc.commands.iter().zip(present) {
                if !present {
                    missing.push(format!("{} (origin {g} {c})", rpc.full_name));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "these methods have no command; extend the generator so it makes one, or list the \
         method under [[no_command]] in generator/commands.toml with the reason:\n  {}",
        missing.join("\n  ")
    );
    assert!(
        stale.is_empty(),
        "commands.toml says these methods have no command, but they have one: {stale:?}"
    );
}

#[test]
fn every_generated_command_has_a_method() {
    let rpcs = rpcs();
    for name in groups(&pool()) {
        let offered: BTreeSet<&str> = rpcs
            .iter()
            .flat_map(|r| r.commands.iter())
            .filter(|(g, _)| *g == name)
            .map(|(_, c)| c.as_str())
            .collect();
        let extra: Vec<String> = group(&name)
            .get_subcommands()
            .map(|c| c.get_name().to_owned())
            .filter(|c| !offered.contains(c.as_str()))
            .collect();
        assert!(
            extra.is_empty(),
            "these commands of `origin {name}` have no method in the API: {extra:?}"
        );
    }
}

/// Every service of the API has its group, which has `--endpoint`; the
/// operations package is no group of its own.
#[test]
fn every_service_has_a_group() {
    let groups = groups(&pool());
    assert!(groups.iter().any(|g| g == "pvs"), "{groups:?}");
    assert!(groups.iter().any(|g| g == "location"), "{groups:?}");
    for name in &groups {
        let group = group(name);
        let endpoint = group
            .get_arguments()
            .find(|a| a.get_long() == Some("endpoint"))
            .unwrap_or_else(|| panic!("`origin {name}` has no --endpoint"));
        // Each group's own variable: redirecting one service leaves the
        // others alone.
        assert_eq!(
            endpoint.get_env().and_then(|e| e.to_str()),
            Some(format!("ORIGINCLOUD_{}_ENDPOINT", name.to_uppercase()).as_str()),
            "origin {name} --endpoint"
        );
    }
    assert!(origincloud_cli::command()
        .find_subcommand(OPERATIONS)
        .is_none());
    assert_eq!(operation_groups(&pool()), ["pvs"]);
}

/// Every entry of the table names a service and a method of the API, and a
/// field of its request, so that none silently stops applying; and every
/// method left without a command says why.
#[test]
fn every_table_entry_names_a_method_and_a_field() {
    let pool = pool();
    let methods = methods(&pool);
    let table = table();
    let groups = groups(&pool);
    for entry in &table.service {
        assert!(
            groups.contains(&entry.name),
            "commands.toml describes the service {}, which the API doesn't have",
            entry.name
        );
    }
    let entries = table
        .no_command
        .iter()
        .map(|n| (&n.service, &n.method, ""))
        .chain(
            table
                .exclude
                .iter()
                .chain(&table.one_way)
                .map(|e| (&e.service, &e.method, e.field.as_str())),
        );
    for (service, method_name, field) in entries {
        let (_, method) = methods
            .iter()
            .find(|(s, m)| s == service && m.name() == method_name)
            .unwrap_or_else(|| {
                panic!("commands.toml names {service} {method_name}, which the API doesn't have")
            });
        if field.is_empty() {
            continue;
        }
        let mut message = method.input();
        let mut parts = field.split('.').peekable();
        while let Some(part) = parts.next() {
            let f = message.get_field_by_name(part).unwrap_or_else(|| {
                panic!("commands.toml names {service} {method_name} {field}, which the API doesn't have")
            });
            if parts.peek().is_some() {
                let Kind::Message(next) = f.kind() else {
                    panic!("{service} {method_name} {field}: {part} isn't a message");
                };
                message = next;
            }
        }
    }
    for n in &table.no_command {
        assert!(
            !n.why.trim().is_empty(),
            "{} {} says no why",
            n.service,
            n.method
        );
    }
}

/// An excluded field has no flag; a one-way field has one, and its command
/// takes `--yes`.
#[test]
fn the_table_shapes_the_commands() {
    let table = table();
    for entry in &table.exclude {
        let group = group(&entry.service);
        let command = group.find_subcommand(entry.method.to_kebab_case()).unwrap();
        let flag = entry.field.rsplit('.').next().unwrap().to_kebab_case();
        assert!(
            !has_flag(command, &flag),
            "origin {} {} has --{flag}, which commands.toml excludes",
            entry.service,
            command.get_name()
        );
    }
    for entry in &table.one_way {
        let group = group(&entry.service);
        let command = group.find_subcommand(entry.method.to_kebab_case()).unwrap();
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

/// The generated code of the group `name`.
fn generated(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/sdk/generated")
        .join(format!("{name}.rs"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The text of the generated function `name`'s signature in `code`.
fn signature<'a>(code: &'a str, name: &str) -> &'a str {
    let start = code
        .find(&format!("async fn {name}("))
        .unwrap_or_else(|| panic!("no generated function {name}"));
    let end = start + code[start..].find('{').unwrap();
    &code[start..end]
}

/// A delete, and a one-way change, reach the SDK only through a wrapper
/// that takes the client only `--yes` makes; and the SDK's method is called
/// nowhere else. Every other method, the operation methods among them, goes
/// through the client that changes nothing irreversibly.
#[test]
fn deletes_and_one_way_changes_need_the_confirmed_client() {
    let table = table();
    let mut checked = 0;
    for rpc in rpcs() {
        for (name, _) in &rpc.commands {
            let code = generated(name);
            let snake = rpc.method.to_snake_case();
            let calls = code.matches(&format!(".{snake}(request)")).count();
            let one_way = table
                .one_way
                .iter()
                .any(|o| o.service == rpc.service && o.method == rpc.method);
            if rpc.method.starts_with("Delete") {
                assert!(
                    signature(&code, &format!("call_{snake}")).contains("&ConfirmedClient"),
                    "call_{snake} doesn't take the confirmed client"
                );
                assert_eq!(calls, 1, "{snake} is called outside its wrapper");
            } else if one_way {
                assert!(signature(&code, &format!("call_{snake}_confirmed"))
                    .contains("&ConfirmedClient"));
                assert!(signature(&code, &format!("call_{snake}")).contains("&ReadClient"));
                assert_eq!(calls, 2, "{snake} is called outside its wrappers");
            } else {
                assert!(signature(&code, &format!("call_{snake}")).contains("&ReadClient"));
                assert_eq!(calls, 1, "{snake} is called outside its wrapper");
            }
            checked += 1;
        }
    }
    assert!(checked >= 21, "only {checked} commands checked");
}

/// The hand-written commands follow the same rule: every irreversible one
/// takes `--yes`.
#[test]
fn every_hand_written_destructive_command_requires_yes() {
    let generated = groups(&pool());
    let root = origincloud_cli::command();
    let mut checked = 0;
    for group in root
        .get_subcommands()
        .filter(|g| !generated.iter().any(|name| name == g.get_name()))
    {
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
