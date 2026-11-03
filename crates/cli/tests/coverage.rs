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

use common::{
    extension, groups, methods, operation_groups, pattern_of_type, pool, OPERATIONS, TABLE,
};
use heck::{ToKebabCase, ToSnakeCase};
use prost_reflect::Kind;
use serde::Deserialize;

#[derive(Deserialize)]
struct Table {
    #[serde(default)]
    hand_written: Vec<String>,
    #[serde(default)]
    replacement: Vec<Replacement>,
    #[serde(default)]
    retired: Vec<Retired>,
    #[serde(default)]
    service: Vec<Group>,
    #[serde(default)]
    no_command: Vec<NoCommand>,
    #[serde(default)]
    exclude: Vec<Entry>,
    #[serde(default)]
    one_way: Vec<Entry>,
    #[serde(default)]
    shown_once: Vec<Entry>,
    #[serde(default)]
    also_names: Vec<AlsoNames>,
}

#[derive(Deserialize)]
struct AlsoNames {
    field: String,
    resource: String,
    why: String,
}

#[derive(Deserialize)]
struct Group {
    name: String,
}

#[derive(Deserialize)]
struct Replacement {
    group: String,
    service: String,
}

#[derive(Deserialize)]
struct Retired {
    group: String,
    #[serde(default)]
    command: String,
    why: String,
}

impl Retired {
    fn path(&self) -> Vec<String> {
        std::iter::once(self.group.as_str())
            .chain(self.command.split_whitespace())
            .map(str::to_owned)
            .collect()
    }
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
    // pvs answers with operations; a new service may too.
    assert!(operation_groups(&pool()).iter().any(|g| g == "pvs"));
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
    // A secret shown once is a field of the response.
    for entry in &table.shown_once {
        let (_, method) = methods
            .iter()
            .find(|(s, m)| *s == entry.service && m.name() == entry.method)
            .unwrap_or_else(|| {
                panic!(
                    "commands.toml names {} {}, which the API doesn't have",
                    entry.service, entry.method
                )
            });
        assert!(
            method.output().get_field_by_name(&entry.field).is_some(),
            "commands.toml names {} {} {}, which its response doesn't have",
            entry.service,
            entry.method,
            entry.field
        );
    }
    for n in &table.no_command {
        assert!(
            !n.why.trim().is_empty(),
            "{} {} says no why",
            n.service,
            n.method
        );
    }
    // A field taking another type's names too is a reference to a type the
    // API declares, and the other type is one.
    for entry in &table.also_names {
        let (message, name) = entry.field.rsplit_once('.').unwrap();
        let field = pool
            .get_message_by_name(message)
            .and_then(|m| m.get_field_by_name(name))
            .unwrap_or_else(|| {
                panic!(
                    "commands.toml names the field {}, which the API doesn't have",
                    entry.field
                )
            });
        let referenced = extension(&pool, &field.options(), "google.api.resource_reference")
            .and_then(|r| r.get_field_by_name("type")?.as_str().map(str::to_owned))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| panic!("{} is no reference to a resource type", entry.field));
        pattern_of_type(&pool, &referenced);
        pattern_of_type(&pool, &entry.resource);
        assert!(!entry.why.trim().is_empty(), "{} says no why", entry.field);
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

/// Whether the hand-written command `name` can't be undone.
fn destructive(name: &str) -> bool {
    name.starts_with("delete-")
        || name.starts_with("terminate-")
        || name == "release-floating-ip"
        || name == "revoke-service-account"
}

/// The hand-written commands follow the same rule: every irreversible one
/// takes `--yes`.
#[test]
fn every_hand_written_destructive_command_requires_yes() {
    let mut checked = 0;
    for (path, command) in hand_written_commands() {
        if destructive(path.last().unwrap()) {
            checked += 1;
            assert!(
                has_flag(&command, "yes"),
                "origin {} has no --yes",
                path.join(" ")
            );
        }
    }
    // As many as the snapshot's destructive commands of the groups still
    // hand-written, none once the last group calling the GraphQL API is
    // retired: the test reads the commands it is meant to.
    let groups = hand_written_groups();
    let retired: Vec<Vec<String>> = table().retired.iter().map(Retired::path).collect();
    let expected = snapshot()
        .into_iter()
        .filter(|p| groups.contains(&p[0]) && !retired.contains(p))
        .filter(|p| destructive(p.last().unwrap()))
        .count();
    assert_eq!(checked, expected, "destructive hand-written commands");
}

/// The groups the generated ones aren't: the hand-written groups, and
/// clap's `help`.
fn hand_written_groups() -> BTreeSet<String> {
    let mut generated = groups(&pool());
    if cfg!(feature = "test-services") {
        generated.push("scratch".to_owned());
    }
    origincloud_cli::command()
        .get_subcommands()
        .map(|g| g.get_name().to_owned())
        .filter(|g| g != "help" && !generated.contains(g))
        .collect()
}

/// Every command the hand-written groups have had, from
/// `hand_written.txt`: `["admin", "create-organisation"]`, `["version"]`.
fn snapshot() -> Vec<Vec<String>> {
    include_str!("hand_written.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.split_whitespace().map(str::to_owned).collect())
        .collect()
}

/// The commands below `command`, to the ones with no subcommands, each
/// with its path from the root.
fn leaves(command: &clap::Command, path: Vec<String>, out: &mut Vec<(Vec<String>, clap::Command)>) {
    let mut subcommands = command
        .get_subcommands()
        .filter(|c| c.get_name() != "help")
        .peekable();
    if subcommands.peek().is_none() {
        out.push((path, command.clone()));
        return;
    }
    for sub in subcommands {
        let mut path = path.clone();
        path.push(sub.get_name().to_owned());
        leaves(sub, path, out);
    }
}

/// The commands of the hand-written groups, by path.
fn hand_written_commands() -> Vec<(Vec<String>, clap::Command)> {
    let root = origincloud_cli::command();
    let mut out = Vec::new();
    for name in hand_written_groups() {
        leaves(root.find_subcommand(&name).unwrap(), vec![name], &mut out);
    }
    out
}

/// Whether `origin {path}` is a command, hand-written or generated.
fn exists(path: &[String]) -> bool {
    let mut command = origincloud_cli::command();
    for name in path {
        match command.find_subcommand(name) {
            Some(sub) => command = sub.clone(),
            None => return false,
        }
    }
    true
}

/// `hand_written` in the table is the CLI's hand-written groups, so that a
/// service named like one, or replacing one, fails generation until it is
/// retired. No hand-written command disappears unnoticed: each one the
/// snapshot lists is still a command, hand-written or generated under the
/// same name, or the table retires it with the reason; and the snapshot
/// lists every hand-written command.
#[test]
fn the_table_knows_the_hand_written_groups() {
    let table = table();
    let listed: BTreeSet<String> = table.hand_written.iter().cloned().collect();
    assert_eq!(
        listed,
        hand_written_groups(),
        "hand_written in generator/commands.toml isn't the CLI's hand-written groups"
    );

    let snapshot = snapshot();
    assert!(snapshot.len() > 1, "hand_written.txt lists nothing");
    let retired: Vec<Vec<String>> = table.retired.iter().map(Retired::path).collect();
    for (path, _) in hand_written_commands() {
        assert!(
            snapshot.contains(&path),
            "origin {} isn't in crates/cli/tests/hand_written.txt: add it",
            path.join(" ")
        );
    }
    for path in &snapshot {
        assert!(
            exists(path) || retired.contains(path),
            "origin {} is gone: give it a [[retired]] entry in generator/commands.toml, with \
             the reason and what to use instead",
            path.join(" ")
        );
    }
    for (entry, path) in table.retired.iter().zip(&retired) {
        let name = path.join(" ");
        assert!(
            snapshot.contains(path),
            "commands.toml retires origin {name}, which hand_written.txt doesn't list"
        );
        assert!(
            !exists(path),
            "commands.toml retires origin {name}, which the CLI still has"
        );
        assert!(!entry.why.trim().is_empty(), "{name} says no why");
    }

    let generated = groups(&pool());
    for replacement in &table.replacement {
        assert!(
            snapshot.iter().any(|p| p[0] == replacement.group),
            "commands.toml says {} replaces origin {}, which hand_written.txt doesn't list",
            replacement.service,
            replacement.group
        );
        assert!(
            !(generated.contains(&replacement.service) && listed.contains(&replacement.group)),
            "{} replaces origin {}, which is still hand-written",
            replacement.service,
            replacement.group
        );
    }
}

/// A package with several services has a [[service]] entry for its group's
/// help: it has no one comment to take it from.
#[test]
fn a_group_of_several_services_has_its_help() {
    let pool = pool();
    let table = table();
    for name in groups(&pool) {
        let services = pool
            .services()
            .filter(|s| common::service_of(s.package_name()) == name)
            .count();
        if services > 1 {
            assert!(
                table.service.iter().any(|g| g.name == name),
                "origin {name} has {services} services: give its help a [[service]] entry, \
                 name = {name:?}, in generator/commands.toml"
            );
        }
    }
}

#[test]
fn the_command_tree_is_valid() {
    origincloud_cli::command().debug_assert();
}
