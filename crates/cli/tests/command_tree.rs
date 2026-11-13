//! `origin __commands`, the command tree each release attaches as
//! `commands.json`: tools outside this repository read it by its field
//! names, so its shape is fixed here, field by field. It lists every
//! visible command with the flags clap gives it, and names the method of
//! every generated command as the coverage test finds it in the API's
//! descriptors.

mod common;

use std::collections::BTreeSet;

use common::{methods, operation_groups, pool, starts_an_operation, OPERATIONS, TABLE};
use heck::ToKebabCase;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tree {
    cli_version: String,
    api_revision: Option<String>,
    commands: Vec<Command>,
    global_args: Vec<Arg>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    path: Vec<String>,
    about: String,
    method: Option<String>,
    long_running: Option<bool>,
    args: Vec<Arg>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arg {
    long: Option<String>,
    short: Option<String>,
    value_name: Option<String>,
    required: bool,
    multiple: bool,
    global: bool,
    positional: bool,
    help: String,
    possible_values: Vec<String>,
}

impl Arg {
    /// Its flag, or its value's name for a positional one.
    fn key(&self) -> String {
        match (&self.long, &self.value_name) {
            (Some(long), _) => format!("--{long}"),
            (None, Some(name)) => name.clone(),
            (None, None) => panic!("an argument with neither a flag nor a value name"),
        }
    }
}

fn run(args: &[&str]) -> std::process::Output {
    let home = tempfile::tempdir().unwrap();
    std::process::Command::new(env!("CARGO_BIN_EXE_origin"))
        .args(args)
        .env_clear()
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .env("XDG_DATA_HOME", home.path())
        .output()
        .expect("origin runs")
}

fn printed() -> String {
    let output = run(&["__commands"]);
    assert!(
        output.status.success(),
        "origin __commands failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("the tree is UTF-8")
}

fn tree() -> Tree {
    serde_json::from_str(&printed()).expect("the tree has the fields tools read, and no others")
}

/// Every visible command below `command`, in clap's order, by path.
fn visible(command: &clap::Command, path: Vec<String>, out: &mut Vec<Vec<String>>) {
    for sub in command
        .get_subcommands()
        .filter(|c| !c.is_hide_set() && c.get_name() != "help")
    {
        let mut path = path.clone();
        path.push(sub.get_name().to_owned());
        out.push(path.clone());
        visible(sub, path, out);
    }
}

/// The flags clap accepts on `origin {path}`, its groups' global ones
/// included, but `origin`'s own and `--help` and `--version`.
fn accepted(root: &clap::Command, path: &[String]) -> BTreeSet<String> {
    let own: BTreeSet<&str> = root.get_arguments().map(|a| a.get_id().as_str()).collect();
    let mut command = root;
    for name in path {
        command = command.find_subcommand(name).unwrap();
    }
    command
        .get_arguments()
        .filter(|a| !a.is_hide_set() && !own.contains(a.get_id().as_str()))
        .filter(|a| {
            !matches!(
                a.get_action(),
                clap::ArgAction::Help | clap::ArgAction::Version
            )
        })
        .map(|a| match a.get_long() {
            Some(long) => format!("--{long}"),
            None => a
                .get_value_names()
                .and_then(|n| n.first())
                .map_or_else(|| a.get_id().as_str().to_uppercase(), ToString::to_string),
        })
        .collect()
}

#[test]
fn it_is_the_tree_of_this_version() {
    let tree = tree();
    assert_eq!(tree.cli_version, env!("CARGO_PKG_VERSION"));
    let revision = include_str!("../../../API_REVISION").trim();
    assert_eq!(tree.api_revision.as_deref(), Some(revision));
    assert_eq!(printed(), printed(), "two runs print different trees");
}

/// Every visible command, groups and leaves, in clap's order; a group lists
/// no flags, and a leaf every flag clap accepts on it.
#[test]
fn it_lists_every_visible_command_and_its_flags() {
    let tree = tree();
    let root = origincloud_cli::command();
    let mut expected = Vec::new();
    visible(&root, Vec::new(), &mut expected);
    let paths: Vec<Vec<String>> = tree.commands.iter().map(|c| c.path.clone()).collect();
    assert_eq!(paths, expected);
    assert!(
        paths.len() > 100,
        "only {} commands: the tree isn't the CLI's",
        paths.len()
    );

    // Clap's own propagation: each command has its groups' global flags.
    let mut built = origincloud_cli::command();
    built.build();
    for command in &tree.commands {
        let name = format!("origin {}", command.path.join(" "));
        assert!(!command.about.is_empty(), "{name} has no about");
        assert!(
            !command.about.contains('\n'),
            "{name}'s about isn't one line"
        );
        let args: BTreeSet<String> = command.args.iter().map(Arg::key).collect();
        assert_eq!(args.len(), command.args.len(), "{name} lists a flag twice");
        let leaf = root
            .find_subcommand(&command.path[0])
            .and_then(|c| {
                command.path[1..]
                    .iter()
                    .try_fold(c, |c, n| c.find_subcommand(n))
            })
            .unwrap();
        if leaf.has_subcommands() {
            assert!(args.is_empty(), "the group {name} lists flags");
            assert!(command.method.is_none() && command.long_running.is_none());
        } else {
            assert_eq!(args, accepted(&built, &command.path), "the flags of {name}");
        }
        for arg in &command.args {
            assert!(!arg.help.is_empty(), "{name} {} has no help", arg.key());
            assert!(arg.short.as_ref().is_none_or(|s| s.chars().count() == 1));
            assert_eq!(arg.positional, arg.long.is_none() && arg.short.is_none());
        }
    }

    let global: Vec<String> = tree.global_args.iter().map(Arg::key).collect();
    let own: Vec<String> = root
        .get_arguments()
        .filter(|a| !a.is_hide_set())
        .map(|a| format!("--{}", a.get_long().unwrap()))
        .collect();
    assert_eq!(global, own, "global_args aren't origin's options");
    let output = tree
        .global_args
        .iter()
        .find(|a| a.long.as_deref() == Some("output"))
        .unwrap();
    assert!(output.global && output.short.as_deref() == Some("o"));
    assert_eq!(output.value_name.as_deref(), Some("FORMAT"));
    assert!(output.possible_values.iter().any(|v| v == "json"));

    let create = tree
        .commands
        .iter()
        .find(|c| c.path == ["pvs", "create-volume"])
        .unwrap();
    let flag = |long: &str| {
        create
            .args
            .iter()
            .find(|a| a.long.as_deref() == Some(long))
            .unwrap_or_else(|| panic!("origin pvs create-volume has no --{long}"))
    };
    let size = flag("size-gib");
    assert!(size.required && !size.multiple && !size.global);
    assert_eq!(size.value_name.as_deref(), Some("SIZE_GIB"));
    let tag = flag("tag");
    assert!(tag.multiple && !tag.required);
    let no_wait = flag("no-wait");
    assert!(no_wait.value_name.is_none() && no_wait.possible_values.is_empty());
    assert!(flag("endpoint").global);
}

/// A generated command names the method it calls, the operation methods
/// included, and whether it starts an operation; a hand-written one names
/// none.
#[test]
fn it_names_the_method_of_every_generated_command() {
    let tree = tree();
    let find = |group: &str, command: &str| {
        tree.commands
            .iter()
            .find(|c| c.path == [group, command])
            .unwrap_or_else(|| panic!("origin {group} {command} isn't in the tree"))
    };
    let pool = pool();
    let operation_groups = operation_groups(&pool);
    let mut named = 0;
    for (service, method) in methods(&pool) {
        let groups = if service == OPERATIONS {
            operation_groups.clone()
        } else {
            vec![service]
        };
        for group in groups {
            let Some(command) = tree
                .commands
                .iter()
                .find(|c| c.path == [group.as_str(), method.name().to_kebab_case().as_str()])
            else {
                // A method [[no_command]] leaves out; the coverage test
                // checks each has its reason.
                continue;
            };
            assert_eq!(command.method.as_deref(), Some(method.full_name()));
            assert_eq!(
                command.long_running,
                Some(starts_an_operation(&pool, &method)),
                "origin {}",
                command.path.join(" ")
            );
            named += 1;
        }
    }
    assert!(named > 21, "only {named} commands name their method");

    for command in tree.commands.iter().filter(|c| c.method.is_some()) {
        let waits = command
            .args
            .iter()
            .any(|a| a.long.as_deref() == Some("no-wait"));
        assert_eq!(
            command.long_running,
            Some(waits),
            "origin {} starts an operation, or has --no-wait, but not both",
            command.path.join(" ")
        );
    }
    assert_eq!(
        find("pvs", "create-volume").method.as_deref(),
        Some("origincloud.pvs.v1beta1.VolumeService.CreateVolume")
    );

    let table: toml::Table = toml::from_str(TABLE).unwrap();
    let hand_written: Vec<&str> = table["hand_written"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g.as_str().unwrap())
        .collect();
    for command in &tree.commands {
        if hand_written.contains(&command.path[0].as_str()) {
            assert!(
                command.method.is_none() && command.long_running.is_none(),
                "the hand-written origin {} names a method",
                command.path.join(" ")
            );
        } else if command.path.len() > 1 {
            assert!(
                command.method.is_some(),
                "the generated origin {} names no method",
                command.path.join(" ")
            );
        }
    }
    assert!(find("id", "login").method.is_none());
}

/// No one types it: neither the help nor the completion offers it.
#[test]
fn it_is_no_command_of_the_tree() {
    for args in [
        &["--help"][..],
        &["help"],
        &["completion", "bash"],
        &["completion", "zsh"],
        &["completion", "fish"],
    ] {
        let output = run(args);
        let printed = String::from_utf8_lossy(&output.stdout);
        assert!(
            !printed.is_empty(),
            "origin {} printed nothing",
            args.join(" ")
        );
        assert!(
            !printed.contains("__commands"),
            "origin {} offers __commands",
            args.join(" ")
        );
    }
    assert!(!run(&["__commands", "extra"]).status.success());
}
