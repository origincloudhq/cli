//! `origin mcp install` and `origin mcp uninstall`: the entry starting
//! `origin mcp serve`, added to or removed from the coding agents of the
//! table in `agents.rs`.
//!
//! An agent with its own command for this, such as `claude mcp add`, gets
//! it run when the program is on `PATH`: the agent then writes its file
//! itself, in whatever form its version reads, and a running agent doesn't
//! race the edit. The file is read before (an entry of the name that isn't
//! `origin mcp serve` is someone else's, and left alone) and after (the
//! entry must be where the table says the agent keeps it). Without the
//! program, and for the agents that have no such command, the file is
//! edited (`config_file.rs`), after a copy of it is written beside it.

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context};
use serde::Serialize;
use serde_json::Value;

use super::agents::{self, Agent, Host, Scope, AGENTS};
use super::commands::{InstallArgs, McpCommands, ServeFlags, Target, UninstallArgs};
use super::config_file::{self, Change, Entry, Found};
use crate::output::{SerializableOutput, SerializationError};

/// The name of the entry without `--name`: the name the server gives
/// itself, which the README's examples use too.
pub(crate) const DEFAULT_NAME: &str = "origincloud";

/// The binary's name, as an entry names it without `--absolute-path`.
const BINARY: &str = "origin";

/// The suffix of the copy of an agent's file made before an edit.
const BACKUP_SUFFIX: &str = ".origin.bak";

/// Runs `origin mcp install` or `uninstall`; `config_file` is the file the
/// global `--config` named, which the entry passes on.
pub(crate) fn run_install_command(
    config_file: Option<&Path>,
    cmd: McpCommands,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let host = Host::current()?;
    match cmd {
        McpCommands::Install(args) => install(&host, config_file, args),
        McpCommands::Uninstall(args) => uninstall(&host, args),
        McpCommands::Serve(_) => unreachable!("`origin mcp serve` is served, not installed"),
    }
}

fn install(
    host: &Host,
    config_file: Option<&Path>,
    args: InstallArgs,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    check_name(&args.target.name)?;
    let serve_args = serve_args(config_file, &args.serve)?;
    if args.print {
        return print(host, &args, serve_args);
    }
    let agents = agents_of(host, &args.target, "add `origin mcp serve` to")?;
    let mut outcomes = Vec::new();
    for agent in agents {
        let entry = Entry {
            name: args.target.name.clone(),
            command: command(host, args.absolute_path || agent.gui)?,
            args: serve_args.clone(),
        };
        let outcome = install_one(host, agent, args.target.scope, &entry);
        outcomes.push(stop_at(agent, outcome, &outcomes)?);
    }
    Ok(Box::new(Outcomes(outcomes)))
}

fn uninstall(host: &Host, args: UninstallArgs) -> anyhow::Result<Box<dyn SerializableOutput>> {
    check_name(&args.target.name)?;
    let agents = agents_of(host, &args.target, "remove `origin mcp serve` from")?;
    let mut outcomes = Vec::new();
    for agent in agents {
        let outcome = uninstall_one(host, agent, args.target.scope, &args.target.name);
        outcomes.push(stop_at(agent, outcome, &outcomes)?);
    }
    Ok(Box::new(Outcomes(outcomes)))
}

/// `outcome`, or its error naming the agent, and the agents done before
/// it, which the error would otherwise leave unsaid.
fn stop_at(
    agent: &Agent,
    outcome: anyhow::Result<Outcome>,
    done: &[Outcome],
) -> anyhow::Result<Outcome> {
    outcome.map_err(|e| {
        let done: Vec<String> = done
            .iter()
            .map(|o| format!("{} {}", o.agent, o.result))
            .collect();
        let done = if done.is_empty() {
            String::new()
        } else {
            format!(" (before it: {})", done.join(", "))
        };
        anyhow::anyhow!("{}: {e:#}{done}", agent.id)
    })
}

/// `--print`: the entry for one agent's file, or in the `mcpServers` form
/// without `--agent`.
fn print(
    host: &Host,
    args: &InstallArgs,
    serve_args: Vec<String>,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let (format, gui) = match args.target.agents.as_slice() {
        [] => (agents::GENERIC, false),
        [id] => {
            let agent = agents::agent(id).expect("clap checked the value");
            // The scope must be one the agent has, as an install's.
            agent.file(args.target.scope, host)?;
            (agent.mcp.format, agent.gui)
        }
        _ => bail!("--print prints the entry for one agent: name one --agent, or none"),
    };
    let entry = Entry {
        name: args.target.name.clone(),
        command: command(host, args.absolute_path || gui)?,
        args: serve_args,
    };
    Ok(Box::new(Printed(config_file::snippet(format, &entry))))
}

/// A name every agent's file takes as a key, and every agent's command as
/// an argument.
fn check_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("--name {name:?}: a name is letters, digits, `-` and `_`");
    }
    Ok(())
}

/// The arguments the entry starts `origin` with: `--config` when it was
/// given, then `mcp serve` and its flags. Never a credential: the server
/// reads those from the login and the environment, as every command does.
pub(crate) fn serve_args(
    config_file: Option<&Path>,
    flags: &ServeFlags,
) -> anyhow::Result<Vec<String>> {
    let mut args = Vec::new();
    if let Some(file) = config_file {
        let file =
            std::path::absolute(file).with_context(|| format!("--config {}", file.display()))?;
        args.push("--config".to_owned());
        args.push(
            file.to_str()
                .with_context(|| format!("--config {}: not UTF-8", file.display()))?
                .to_owned(),
        );
    }
    args.extend(["mcp".to_owned(), "serve".to_owned()]);
    if !flags.toolsets.is_empty() {
        args.push("--toolsets".to_owned());
        args.push(flags.toolsets.join(","));
    }
    if flags.read_only {
        args.push("--read-only".to_owned());
    }
    Ok(args)
}

/// The command of the entry: `origin`, for the agent to find on its
/// `PATH`, or `absolute`ly, the binary's path.
fn command(host: &Host, absolute: bool) -> anyhow::Result<String> {
    if !absolute {
        if host.which(BINARY).is_none() {
            eprintln!(
                "warning: `{BINARY}` isn't on PATH, so an agent started from this shell won't \
                 find it: add its directory to PATH, or use --absolute-path"
            );
        }
        return Ok(BINARY.to_owned());
    }
    let exe = std::env::current_exe().context("the path of the `origin` binary")?;
    // The binary as PATH has it, when that is this one: a link a package
    // manager keeps (`/opt/homebrew/bin/origin`) outlives an upgrade, the
    // versioned path it points to doesn't.
    let real = exe.canonicalize().unwrap_or_else(|_| exe.clone());
    let path = match host.which(BINARY) {
        Some(on_path) if on_path.canonicalize().ok().as_ref() == Some(&real) => on_path,
        _ => exe,
    };
    path.to_str()
        .map(str::to_owned)
        .with_context(|| format!("the path of `origin`, {}, isn't UTF-8", path.display()))
}

/// The agents `--agent` names; without it, on a terminal, those chosen of
/// the ones found on this machine.
fn agents_of(host: &Host, target: &Target, verb: &str) -> anyhow::Result<Vec<&'static Agent>> {
    if !target.agents.is_empty() {
        let mut chosen: Vec<&'static Agent> = Vec::new();
        for id in &target.agents {
            let agent = agents::agent(id).expect("clap checked the value");
            if !chosen.iter().any(|a| a.id == agent.id) {
                chosen.push(agent);
            }
        }
        return Ok(chosen);
    }
    let ids = AGENTS.iter().map(|a| a.id).collect::<Vec<_>>().join(", ");
    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        bail!("name the agent with --agent ({ids}): without a terminal there is no one to ask");
    }
    let found: Vec<&'static Agent> = AGENTS.iter().filter(|a| a.detected(host)).collect();
    if found.is_empty() {
        bail!("found none of the agents on this machine: name one with --agent ({ids})");
    }
    let mut stderr = std::io::stderr();
    writeln!(stderr, "Agents found on this machine:")?;
    for (i, agent) in found.iter().enumerate() {
        let file = agent
            .file(target.scope, host)
            .map(|f| f.display().to_string())
            .unwrap_or_else(|_| format!("no {} scope", target.scope.as_str()));
        writeln!(stderr, "  {}. {} ({})  {file}", i + 1, agent.name, agent.id)?;
    }
    write!(
        stderr,
        "Which to {verb}, at the {} scope? Numbers separated by commas, Enter for all: ",
        target.scope.as_str()
    )?;
    stderr.flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let picked = choice(&line, found.len())?;
    if picked.is_empty() {
        bail!("no agent chosen");
    }
    Ok(picked.into_iter().map(|i| found[i]).collect())
}

/// The indices an answer to the prompt picks of `count` agents: Enter
/// for all, else 1-based numbers separated by commas or spaces.
fn choice(answer: &str, count: usize) -> anyhow::Result<Vec<usize>> {
    let answer = answer.trim();
    if answer.is_empty() || answer.eq_ignore_ascii_case("all") {
        return Ok((0..count).collect());
    }
    let mut picked = Vec::new();
    for word in answer.split([',', ' ']).filter(|w| !w.is_empty()) {
        match word.parse::<usize>() {
            Ok(n) if (1..=count).contains(&n) => {
                if !picked.contains(&(n - 1)) {
                    picked.push(n - 1);
                }
            }
            _ => bail!("{word:?} isn't one of the numbers 1 to {count}"),
        }
    }
    Ok(picked)
}

/// What install or uninstall did for one agent.
#[derive(Debug, Serialize)]
pub(crate) struct Outcome {
    agent: &'static str,
    scope: &'static str,
    name: String,
    file: String,
    /// `file`, or the agent's command that made the change.
    by: String,
    /// `added`, `updated`, `unchanged`, `removed` or `absent`.
    result: &'static str,
    /// The copy of the file before the edit.
    #[serde(skip_serializing_if = "Option::is_none")]
    backup: Option<String>,
}

fn describe(change: Change) -> &'static str {
    match change {
        Change::Added => "added",
        Change::Updated => "updated",
        Change::Unchanged => "unchanged",
        Change::Removed => "removed",
        Change::Absent => "absent",
    }
}

/// The file's content, or `None` when it doesn't exist.
fn read(file: &Path) -> anyhow::Result<Option<String>> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", file.display())),
    }
}

/// The entry `name` in `file`, failing for a file that doesn't parse.
fn find(
    agent: &Agent,
    file: &Path,
    text: Option<&str>,
    name: &str,
) -> anyhow::Result<Option<Found>> {
    let Some(text) = text else {
        return Ok(None);
    };
    config_file::find(agent.mcp.format, text, name).map_err(|e| unparseable(file, e))
}

fn unparseable(file: &Path, e: config_file::Invalid) -> anyhow::Error {
    anyhow::anyhow!(
        "{} {e}: fix it, or move it away, and run this again; it was left as it is",
        file.display()
    )
}

/// Refuses an entry of the name that doesn't start `origin mcp serve`.
fn ours(found: &Found, file: &Path, name: &str) -> anyhow::Result<()> {
    if !found.is_origin() {
        bail!(
            "{} has a server named {name:?} that doesn't start `origin mcp serve`: it was left \
             as it is; choose another --name",
            file.display()
        );
    }
    Ok(())
}

fn install_one(
    host: &Host,
    agent: &'static Agent,
    scope: Scope,
    entry: &Entry,
) -> anyhow::Result<Outcome> {
    let file = agent.file(scope, host)?;
    let text = read(&file)?;
    let found = find(agent, &file, text.as_deref(), &entry.name)?;
    if let Some(found) = &found {
        ours(found, &file, &entry.name)?;
    }
    let outcome = |by: String, change, backup| Outcome {
        agent: agent.id,
        scope: scope.as_str(),
        name: entry.name.clone(),
        file: file.display().to_string(),
        by,
        result: describe(change),
        backup,
    };

    if let Some((command, program)) = agent
        .command(scope)
        .and_then(|c| host.which(c.program()).map(|p| (c, p)))
    {
        if found
            .as_ref()
            .is_some_and(|f| f.is(entry, agent.mcp.format))
        {
            return Ok(outcome(command.describe("add"), Change::Unchanged, None));
        }
        // Some agents refuse to add a name they have; removing it first
        // makes the add an update.
        if found.is_some() {
            run(host, &program, &command.remove(scope, &entry.name))?;
        }
        run(
            host,
            &program,
            &command.add(scope, &entry.name, &entry.command, &entry.args),
        )?;
        let after = find(agent, &file, read(&file)?.as_deref(), &entry.name)?;
        if !after.is_some_and(|f| {
            f.command.as_deref() == Some(&entry.command) && f.args.as_deref() == Some(&entry.args)
        }) {
            bail!(
                "`{} {}` succeeded, but {} doesn't have the entry {:?} it should have added",
                command.program(),
                command
                    .add(scope, &entry.name, &entry.command, &entry.args)
                    .join(" "),
                file.display(),
                entry.name
            );
        }
        let change = if found.is_some() {
            Change::Updated
        } else {
            Change::Added
        };
        return Ok(outcome(command.describe("add"), change, None));
    }

    let (edited, change) = config_file::install(agent.mcp.format, text.as_deref(), entry)
        .map_err(|e| unparseable(&file, e))?;
    let backup = write(&file, text.as_deref(), &edited, change)?;
    Ok(outcome("file".to_owned(), change, backup))
}

fn uninstall_one(
    host: &Host,
    agent: &'static Agent,
    scope: Scope,
    name: &str,
) -> anyhow::Result<Outcome> {
    let file = agent.file(scope, host)?;
    let text = read(&file)?;
    let found = find(agent, &file, text.as_deref(), name)?;
    let outcome = |by: String, change, backup| Outcome {
        agent: agent.id,
        scope: scope.as_str(),
        name: name.to_owned(),
        file: file.display().to_string(),
        by,
        result: describe(change),
        backup,
    };
    let (Some(found), Some(text)) = (found, text) else {
        return Ok(outcome("file".to_owned(), Change::Absent, None));
    };
    ours(&found, &file, name)?;

    if let Some((command, program)) = agent
        .command(scope)
        .and_then(|c| host.which(c.program()).map(|p| (c, p)))
    {
        run(host, &program, &command.remove(scope, name))?;
        if find(agent, &file, read(&file)?.as_deref(), name)?.is_some() {
            bail!(
                "`{} {}` succeeded, but {} still has the entry {name:?}",
                command.program(),
                command.remove(scope, name).join(" "),
                file.display()
            );
        }
        let by = command.describe("remove");
        return Ok(outcome(by, Change::Removed, None));
    }

    let (edited, change) =
        config_file::uninstall(agent.mcp.format, &text, name).map_err(|e| unparseable(&file, e))?;
    let backup = write(&file, Some(&text), &edited, change)?;
    Ok(outcome("file".to_owned(), change, backup))
}

/// Runs an agent's command in the current directory, where its project
/// scope is.
fn run(host: &Host, program: &Path, args: &[String]) -> anyhow::Result<()> {
    let name = program
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let output = Command::new(program)
        .args(args)
        .current_dir(&host.cwd)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("running {}", program.display()))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        let said = if said.trim().is_empty() {
            String::from_utf8_lossy(&output.stdout)
        } else {
            said
        };
        bail!(
            "`{name} {}` failed ({}): {}",
            args.join(" "),
            output.status,
            said.trim()
        );
    }
    Ok(())
}

/// Writes `edited` to `file` when `change` changed it: first a copy of
/// what it held (`before`), beside it, then the new content to a temporary
/// file renamed over it, so that the file is whole at every moment. The
/// path of the copy, if one was made.
///
/// A `file` that is a symbolic link, such as one into a dotfiles
/// repository, is written through: its target is edited, with the copy and
/// the temporary file beside the target, and the link stays a link. The
/// copy and the temporary file of an existing file are created with its
/// permissions from the start, so that the content of a file only you can
/// read, such as `~/.claude.json`, is never readable by others, not even
/// for a moment.
fn write(
    file: &Path,
    before: Option<&str>,
    edited: &str,
    change: Change,
) -> anyhow::Result<Option<String>> {
    if matches!(change, Change::Unchanged | Change::Absent) {
        return Ok(None);
    }
    let file = resolve_links(file)?;
    let dir = file.parent().context("the file has a directory")?;
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    // The file's permissions, when it exists, which the copy and the new
    // content get.
    let permissions = match before {
        Some(_) => Some(
            std::fs::metadata(&file)
                .with_context(|| format!("reading {}", file.display()))?
                .permissions(),
        ),
        None => None,
    };
    let backup = match before {
        Some(before) => {
            let backup = sibling(&file, BACKUP_SUFFIX);
            create(&backup, before, permissions.as_ref())
                .with_context(|| format!("writing the copy {}", backup.display()))?;
            Some(backup.display().to_string())
        }
        None => None,
    };
    let temporary = sibling(&file, ".origin.tmp");
    create(&temporary, edited, permissions.as_ref())
        .with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, &file).with_context(|| format!("replacing {}", file.display()))?;
    Ok(backup)
}

/// The file `path` names after following its symbolic links, which may
/// be relative to the directory of the link, or dangle.
fn resolve_links(path: &Path) -> anyhow::Result<PathBuf> {
    let mut path = path.to_path_buf();
    // As many as the kernel follows before it gives up (ELOOP).
    for _ in 0..40 {
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = std::fs::read_link(&path)
                    .with_context(|| format!("reading the link {}", path.display()))?;
                path = match path.parent() {
                    Some(dir) if target.is_relative() => dir.join(target),
                    _ => target,
                };
            }
            // Its directory without the `..` a relative link brings, for
            // the paths the output names.
            _ => {
                return Ok(match (path.parent(), path.file_name()) {
                    (Some(dir), Some(name)) => match std::fs::canonicalize(dir) {
                        Ok(dir) => dir.join(name),
                        Err(_) => path,
                    },
                    _ => path,
                })
            }
        }
    }
    bail!("{}: too many levels of symbolic links", path.display())
}

/// Writes `content` to the new file `path`, replacing one left behind by
/// an earlier run; with `permissions`, the file has them from its creation
/// on, never more open than they are.
fn create(
    path: &Path,
    content: &str,
    permissions: Option<&std::fs::Permissions>,
) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if let Some(permissions) = permissions {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        // The umask can only narrow this.
        options.mode(permissions.mode() & 0o7777);
    }
    let mut out = options.open(path)?;
    out.write_all(content.as_bytes())?;
    out.sync_all()?;
    drop(out);
    // Exactly the file's, which the umask may have narrowed.
    if let Some(permissions) = permissions {
        std::fs::set_permissions(path, permissions.clone())?;
    }
    Ok(())
}

fn sibling(file: &Path, suffix: &str) -> PathBuf {
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    file.with_file_name(name)
}

/// The outcome for each agent: a table, or a list in JSON.
struct Outcomes(Vec<Outcome>);

impl SerializableOutput for Outcomes {
    fn to_json(&self) -> Result<Value, SerializationError> {
        serde_json::to_value(&self.0).map_err(|e| SerializationError::Other(e.to_string()))
    }
}

/// `--print`'s snippet, as it is.
struct Printed(String);

impl SerializableOutput for Printed {
    fn to_json(&self) -> Result<Value, SerializationError> {
        Ok(Value::String(self.0.clone()))
    }

    fn to_raw(&self) -> Option<String> {
        Some(self.0.trim_end().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_picks_agents_by_number() {
        assert_eq!(choice("\n", 3).unwrap(), vec![0, 1, 2]);
        assert_eq!(choice("all\n", 2).unwrap(), vec![0, 1]);
        assert_eq!(choice("2, 1 2\n", 3).unwrap(), vec![1, 0]);
        assert!(choice("4", 3).is_err());
        assert!(choice("0", 3).is_err());
        assert!(choice("claude", 3).is_err());
    }

    #[test]
    fn the_entry_passes_the_flags_on() {
        let flags = ServeFlags {
            toolsets: vec!["pvs".to_owned(), "location".to_owned()],
            read_only: true,
        };
        assert_eq!(
            serve_args(None, &flags).unwrap(),
            ["mcp", "serve", "--toolsets", "pvs,location", "--read-only"]
        );
        assert_eq!(
            serve_args(Some(Path::new("/c/p.json")), &ServeFlags::default()).unwrap(),
            ["--config", "/c/p.json", "mcp", "serve"]
        );
    }

    /// The entry is a command line `origin` takes as it is.
    #[test]
    fn the_entry_is_a_command_line_origin_parses() {
        let flags = ServeFlags {
            toolsets: vec!["pvs".to_owned()],
            read_only: true,
        };
        let args = serve_args(Some(Path::new("/c/p.json")), &flags).unwrap();
        let parsed =
            crate::command().try_get_matches_from(std::iter::once("origin".to_owned()).chain(args));
        assert!(parsed.is_ok(), "{parsed:?}");
    }

    #[test]
    fn a_name_is_a_key_every_agent_takes() {
        assert!(check_name("origincloud").is_ok());
        assert!(check_name("origin_cloud-2").is_ok());
        for bad in ["", "a b", "a.b", "ä", "a/b"] {
            assert!(check_name(bad).is_err(), "{bad:?}");
        }
    }
}
