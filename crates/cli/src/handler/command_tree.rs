//! `origin __commands`: the command tree as JSON, for the tools that name
//! `origin`'s commands and flags, such as the agent skills generated from
//! the API. Every release attaches it as `commands.json`, from the binary
//! it publishes (`release/assemble.sh`), so it is the tree of that version.
//!
//! It isn't in the tree itself, so that neither the help nor the shell
//! completion offers it, and takes no options: its output is JSON. It lists
//! every visible command below `origin`, groups and leaves, in the order
//! clap defines them, each with its flags and, for a generated one, the
//! method of the API it calls; `global_args` are `origin`'s own options.

use std::collections::BTreeMap;

use clap::{Arg as ClapArg, ArgAction, Command as ClapCommand};
use heck::ToKebabCase;
use prost_reflect::MethodDescriptor;
use serde::Serialize;

use crate::descriptors;

/// The command, as `origin`'s only argument.
pub(crate) const NAME: &str = "__commands";

/// The package of the operation methods, whose commands are in every group
/// answering with operations, as `GetOperation` is `origin pvs
/// get-operation`.
const OPERATIONS: &str = "longrunning";

#[derive(Serialize)]
struct Tree {
    cli_version: &'static str,
    /// The commit of origincloudhq/api the commands are generated from.
    api_revision: &'static str,
    commands: Vec<Command>,
    global_args: Vec<Arg>,
}

#[derive(Serialize)]
struct Command {
    path: Vec<String>,
    about: String,
    /// The full name of the method a generated command calls; none for a
    /// group or a hand-written command.
    method: Option<String>,
    /// Whether that method starts an operation, which the command waits for
    /// unless `--no-wait`; none without a method.
    long_running: Option<bool>,
    args: Vec<Arg>,
}

#[derive(Serialize)]
struct Arg {
    long: Option<String>,
    short: Option<char>,
    value_name: Option<String>,
    required: bool,
    multiple: bool,
    global: bool,
    /// Given by its position, without a flag, as `origin completion bash`'s
    /// shell.
    positional: bool,
    help: String,
    possible_values: Vec<String>,
}

/// Prints the tree as JSON on standard output.
pub(crate) fn run() -> i32 {
    match serde_json::to_string_pretty(&tree()) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(err) => {
            eprintln!("origin {NAME}: {err}");
            1
        }
    }
}

fn tree() -> Tree {
    let root = crate::command();
    let methods = methods();
    let mut commands = Vec::new();
    for command in visible(&root) {
        walk(
            command,
            vec![command.get_name().to_owned()],
            &[],
            &methods,
            &mut commands,
        );
    }
    Tree {
        cli_version: env!("CARGO_PKG_VERSION"),
        api_revision: include_str!("../../../../API_REVISION").trim(),
        commands,
        global_args: root.get_arguments().filter_map(arg).collect(),
    }
}

/// Adds `command`, at `path`, and the commands below it to `out`. A leaf
/// lists its own flags and the global ones of the groups above it, such as
/// a service's `--endpoint`; a group lists none, as `origin {group}` alone
/// runs nothing.
fn walk(
    command: &ClapCommand,
    path: Vec<String>,
    inherited: &[&ClapArg],
    methods: &BTreeMap<(String, String), MethodDescriptor>,
    out: &mut Vec<Command>,
) {
    let group = command.has_subcommands();
    let method = match path.as_slice() {
        [service, name] if !group => methods.get(&(service.clone(), name.clone())),
        _ => None,
    };
    let args = if group {
        Vec::new()
    } else {
        command
            .get_arguments()
            .chain(inherited.iter().copied())
            .filter_map(arg)
            .collect()
    };
    out.push(Command {
        path: path.clone(),
        about: one_line(command.get_about().map(ToString::to_string)),
        method: method.map(|m| m.full_name().to_owned()),
        long_running: method.map(descriptors::starts_an_operation),
        args,
    });
    let mut inherited = inherited.to_vec();
    inherited.extend(command.get_arguments().filter(|a| a.is_global_set()));
    for sub in visible(command) {
        let mut path = path.clone();
        path.push(sub.get_name().to_owned());
        walk(sub, path, &inherited, methods, out);
    }
}

fn visible(command: &ClapCommand) -> impl Iterator<Item = &ClapCommand> {
    command.get_subcommands().filter(|c| !c.is_hide_set())
}

/// The methods of the generated commands, by group and command, as the
/// generator names them: `origin {service} {method}` in kebab case, and the
/// operation methods in each group with a method starting an operation.
/// The coverage test checks that every generated command is one of them.
fn methods() -> BTreeMap<(String, String), MethodDescriptor> {
    let mut by_command = BTreeMap::new();
    let mut operations = Vec::new();
    let mut operation_groups = Vec::new();
    for service in descriptors::services() {
        let Some(group) = service
            .package_name()
            .strip_prefix("origincloud.")
            .and_then(|p| p.split('.').next())
        else {
            continue;
        };
        for method in service.methods() {
            if group == OPERATIONS {
                operations.push(method);
                continue;
            }
            if descriptors::starts_an_operation(&method) {
                operation_groups.push(group.to_owned());
            }
            by_command.insert((group.to_owned(), method.name().to_kebab_case()), method);
        }
    }
    for group in operation_groups {
        for method in &operations {
            by_command.insert(
                (group.clone(), method.name().to_kebab_case()),
                method.clone(),
            );
        }
    }
    by_command
}

/// `arg` as the tree lists it; none for a hidden one, and for `--help` and
/// `--version`, which every command has.
fn arg(arg: &ClapArg) -> Option<Arg> {
    if arg.is_hide_set() || matches!(arg.get_action(), ArgAction::Help | ArgAction::Version) {
        return None;
    }
    let takes_values = arg.get_action().takes_values();
    let value_name = takes_values.then(|| match arg.get_value_names() {
        Some(names) if !names.is_empty() => names
            .iter()
            .map(|n| n.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        // What the help shows without one: the field's name.
        _ => arg.get_id().as_str().to_uppercase(),
    });
    let possible_values = if takes_values {
        arg.get_possible_values()
            .iter()
            .filter(|v| !v.is_hide_set())
            .map(|v| v.get_name().to_owned())
            .collect()
    } else {
        Vec::new()
    };
    Some(Arg {
        long: arg.get_long().map(str::to_owned),
        short: arg.get_short(),
        value_name,
        required: arg.is_required_set(),
        multiple: matches!(arg.get_action(), ArgAction::Append | ArgAction::Count)
            || arg.get_num_args().is_some_and(|n| n.max_values() > 1),
        global: arg.is_global_set(),
        positional: arg.is_positional(),
        help: one_line(
            arg.get_help()
                .or_else(|| arg.get_long_help())
                .map(ToString::to_string),
        ),
        possible_values,
    })
}

/// The first paragraph of `text`, on one line.
fn one_line(text: Option<String>) -> String {
    let text = text.unwrap_or_default();
    let paragraph = text.split("\n\n").next().unwrap_or_default();
    paragraph.split_whitespace().collect::<Vec<_>>().join(" ")
}
