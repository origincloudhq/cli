//! The coding agents `origin mcp install` registers the MCP server with,
//! and `origin skill install` installs the agent skills for, in one table,
//! [`AGENTS`]: what each is called, how to tell it is installed, and, for
//! each scope, the file its MCP servers are configured in, that file's
//! format and the agent's own command for adding one (`mcp`), and the
//! directory it reads skills from (`skills`).
//!
//! `--agent`'s values and the help of both commands, the README's tables of
//! agents (a test checks they are these) and the tests all read it, so
//! adding an agent is a row here, and a format adapter in
//! `mcp/config_file.rs` when it brings a new format.

use std::ffi::OsString;
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::bail;

/// Where an agent's MCP configuration or skills apply: to every project of
/// the user, or to the project in the current directory, in a file or a
/// directory that is usually committed with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Scope {
    /// Every project of this user, in the agent's own configuration.
    User,
    /// The project in the current directory, in a file beside its code.
    Project,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::User => "user",
            Scope::Project => "project",
        }
    }
}

/// A coding agent, one row of the table.
#[derive(Debug)]
pub(crate) struct Agent {
    /// The value of `--agent`.
    pub id: &'static str,
    /// What the agent is called.
    pub name: &'static str,
    /// Programs on `PATH` that show the agent is installed.
    pub programs: &'static [&'static str],
    /// A desktop application, started without the login shell's `PATH`:
    /// its entry names the `origin` binary by its absolute path.
    pub gui: bool,
    /// How the agent is given an MCP server.
    pub mcp: Mcp,
    /// Where the agent reads agent skills from, if it reads them.
    pub skills: Option<Skills>,
}

/// Where an agent reads agent skills from: a directory holding one
/// directory per skill, each with its `SKILL.md`.
#[derive(Debug)]
pub(crate) struct Skills {
    /// The directory of the user scope.
    pub user: Location,
    /// The directory of the project scope, relative to the project's
    /// directory. Several agents read `.agents/skills`, and share one copy
    /// there.
    pub project: &'static str,
}

/// How an agent is given a local MCP server.
#[derive(Debug)]
pub(crate) struct Mcp {
    /// The file of the user scope.
    pub user: Option<Location>,
    /// The file of the project scope, relative to the project's directory.
    pub project: Option<&'static str>,
    /// The format of both.
    pub format: Format,
    /// The agent's own command for adding and removing a server, used
    /// where it covers the scope and the program is on `PATH`.
    pub command: Option<AgentCommand>,
}

/// A file or directory in a user's directory: `dir`/`file` under `base`,
/// or `file` under the directory the variable `env` names when it is set.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Location {
    pub env: Option<&'static str>,
    pub base: Base,
    pub dir: &'static str,
    pub file: &'static str,
}

/// The directory a [`Location`] is under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Base {
    /// The home directory.
    Home,
    /// Where desktop applications keep their configuration:
    /// `~/Library/Application Support` on macOS, `XDG_CONFIG_HOME` (or
    /// `~/.config`) elsewhere.
    AppConfig,
}

/// The syntax of an agent's configuration file, and where in it the
/// servers are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Format {
    pub syntax: Syntax,
    /// The key of the table of servers, by name.
    pub servers: &'static str,
    /// Whether an entry says `"type": "stdio"`.
    pub stdio_type: bool,
    /// Whether an entry lists the tools it exposes, `"tools": ["*"]`.
    pub all_tools: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Syntax {
    /// JSON, without comments.
    Json,
    /// JSON with comments and trailing commas, which an edit keeps.
    Jsonc,
    /// TOML, whose comments an edit keeps.
    Toml,
}

impl Syntax {
    pub fn describe(self) -> &'static str {
        match self {
            Syntax::Json => "JSON",
            Syntax::Jsonc => "JSON with comments",
            Syntax::Toml => "TOML",
        }
    }
}

/// An agent's own command for adding and removing an MCP server, which
/// `origin mcp install` runs rather than editing the agent's file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentCommand {
    /// `claude mcp add --scope {user|project} NAME -- COMMAND ARGS…`,
    /// `claude mcp remove --scope … NAME`.
    Claude,
    /// `codex mcp add NAME -- COMMAND ARGS…`, `codex mcp remove NAME`: the
    /// user scope only.
    Codex,
    /// `gemini mcp add --scope {user|project} NAME COMMAND -- ARGS…`,
    /// `gemini mcp remove --scope … NAME`.
    Gemini,
}

impl AgentCommand {
    /// The program the command runs.
    pub fn program(self) -> &'static str {
        match self {
            AgentCommand::Claude => "claude",
            AgentCommand::Codex => "codex",
            AgentCommand::Gemini => "gemini",
        }
    }

    /// Whether it can add a server at `scope`.
    pub fn covers(self, scope: Scope) -> bool {
        match self {
            AgentCommand::Claude | AgentCommand::Gemini => true,
            AgentCommand::Codex => scope == Scope::User,
        }
    }

    /// The arguments adding `name`, which runs `command` with `args`.
    pub fn add(self, scope: Scope, name: &str, command: &str, args: &[String]) -> Vec<String> {
        let mut argv: Vec<String> = match self {
            AgentCommand::Claude => {
                vec!["mcp", "add", "--scope", scope.as_str(), name, "--", command]
            }
            AgentCommand::Codex => vec!["mcp", "add", name, "--", command],
            AgentCommand::Gemini => {
                vec!["mcp", "add", "--scope", scope.as_str(), name, command, "--"]
            }
        }
        .into_iter()
        .map(str::to_owned)
        .collect();
        argv.extend(args.iter().cloned());
        argv
    }

    /// The arguments removing `name`.
    pub fn remove(self, scope: Scope, name: &str) -> Vec<String> {
        match self {
            AgentCommand::Claude | AgentCommand::Gemini => {
                vec!["mcp", "remove", "--scope", scope.as_str(), name]
            }
            AgentCommand::Codex => vec!["mcp", "remove", name],
        }
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    /// How the help and the output name its `verb` (`add`, `remove`):
    /// `claude mcp add`.
    pub fn describe(self, verb: &str) -> String {
        format!("{} mcp {verb}", self.program())
    }
}

const JSON_MCP_SERVERS: Format = Format {
    syntax: Syntax::Json,
    servers: "mcpServers",
    stdio_type: false,
    all_tools: false,
};

/// The `mcpServers` form most agents read, which `--print` prints without
/// `--agent`.
pub(crate) const GENERIC: Format = JSON_MCP_SERVERS;

/// The agents, in the order `--help` lists them.
pub(crate) const AGENTS: &[Agent] = &[
    Agent {
        id: "claude-code",
        name: "Claude Code",
        programs: &["claude"],
        gui: false,
        mcp: Mcp {
            user: Some(Location {
                env: Some("CLAUDE_CONFIG_DIR"),
                base: Base::Home,
                dir: "",
                file: ".claude.json",
            }),
            project: Some(".mcp.json"),
            format: Format {
                stdio_type: true,
                ..JSON_MCP_SERVERS
            },
            command: Some(AgentCommand::Claude),
        },
        skills: Some(Skills {
            user: Location {
                env: Some("CLAUDE_CONFIG_DIR"),
                base: Base::Home,
                dir: ".claude",
                file: "skills",
            },
            project: ".claude/skills",
        }),
    },
    Agent {
        id: "codex",
        name: "OpenAI Codex",
        programs: &["codex"],
        gui: false,
        mcp: Mcp {
            user: Some(Location {
                env: Some("CODEX_HOME"),
                base: Base::Home,
                dir: ".codex",
                file: "config.toml",
            }),
            project: Some(".codex/config.toml"),
            format: Format {
                syntax: Syntax::Toml,
                servers: "mcp_servers",
                stdio_type: false,
                all_tools: false,
            },
            command: Some(AgentCommand::Codex),
        },
        skills: Some(Skills {
            user: Location {
                env: None,
                base: Base::Home,
                dir: ".agents",
                file: "skills",
            },
            project: ".agents/skills",
        }),
    },
    Agent {
        id: "cursor",
        name: "Cursor",
        programs: &["cursor", "cursor-agent"],
        gui: false,
        mcp: Mcp {
            user: Some(Location {
                env: None,
                base: Base::Home,
                dir: ".cursor",
                file: "mcp.json",
            }),
            project: Some(".cursor/mcp.json"),
            format: Format {
                stdio_type: true,
                ..JSON_MCP_SERVERS
            },
            command: None,
        },
        skills: Some(Skills {
            user: Location {
                env: None,
                base: Base::Home,
                dir: ".cursor",
                file: "skills",
            },
            project: ".agents/skills",
        }),
    },
    Agent {
        id: "gemini-cli",
        name: "Gemini CLI",
        programs: &["gemini"],
        gui: false,
        mcp: Mcp {
            user: Some(Location {
                env: None,
                base: Base::Home,
                dir: ".gemini",
                file: "settings.json",
            }),
            project: Some(".gemini/settings.json"),
            format: Format {
                syntax: Syntax::Jsonc,
                ..JSON_MCP_SERVERS
            },
            command: Some(AgentCommand::Gemini),
        },
        skills: Some(Skills {
            user: Location {
                env: None,
                base: Base::Home,
                dir: ".gemini",
                file: "skills",
            },
            project: ".agents/skills",
        }),
    },
    Agent {
        id: "github-copilot",
        name: "GitHub Copilot (VS Code, Copilot CLI)",
        programs: &["copilot", "code"],
        gui: false,
        mcp: Mcp {
            user: Some(Location {
                env: Some("COPILOT_HOME"),
                base: Base::Home,
                dir: ".copilot",
                file: "mcp-config.json",
            }),
            project: Some(".mcp.json"),
            format: Format {
                stdio_type: true,
                all_tools: true,
                ..JSON_MCP_SERVERS
            },
            command: None,
        },
        skills: Some(Skills {
            user: Location {
                env: Some("COPILOT_HOME"),
                base: Base::Home,
                dir: ".copilot",
                file: "skills",
            },
            project: ".agents/skills",
        }),
    },
    Agent {
        id: "claude-desktop",
        name: "Claude Desktop",
        programs: &[],
        gui: true,
        mcp: Mcp {
            user: Some(Location {
                env: None,
                base: Base::AppConfig,
                dir: "Claude",
                file: "claude_desktop_config.json",
            }),
            project: None,
            format: JSON_MCP_SERVERS,
            command: None,
        },
        skills: None,
    },
];

/// The agent `id` names.
pub(crate) fn agent(id: &str) -> Option<&'static Agent> {
    AGENTS.iter().find(|a| a.id == id)
}

/// `--agent`'s values, each with the agent's name as its help.
pub(crate) fn possible_values() -> Vec<clap::builder::PossibleValue> {
    AGENTS
        .iter()
        .map(|a| clap::builder::PossibleValue::new(a.id).help(a.name))
        .collect()
}

/// The agents that read agent skills.
pub(crate) fn with_skills() -> impl Iterator<Item = &'static Agent> {
    AGENTS.iter().filter(|a| a.skills.is_some())
}

/// `--agent`'s values for the skills: the agents that read them.
pub(crate) fn skill_possible_values() -> Vec<clap::builder::PossibleValue> {
    with_skills()
        .map(|a| clap::builder::PossibleValue::new(a.id).help(a.name))
        .collect()
}

/// The operating systems whose locations differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Os {
    MacOs,
    /// Linux, and the other Unix systems.
    Unix,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Unix
        }
    }
}

/// What the locations are resolved against: the home directory, the
/// current one, the operating system and the environment. Passed in rather
/// than read where needed, so that the tests resolve against a directory of
/// their own and never the developer's.
#[derive(Debug, Clone)]
pub(crate) struct Host {
    pub home: PathBuf,
    pub cwd: PathBuf,
    pub os: Os,
    /// The variables the locations read (`CODEX_HOME`, `XDG_CONFIG_HOME`),
    /// `XDG_STATE_HOME`, and `PATH`.
    pub vars: Vec<(String, OsString)>,
}

impl Host {
    /// This machine.
    pub fn current() -> anyhow::Result<Self> {
        let home = std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("HOME isn't set, and the agents' files are under it"))?;
        let mut vars: Vec<(String, OsString)> = ["PATH", "XDG_CONFIG_HOME"]
            .into_iter()
            .chain(AGENTS.iter().filter_map(|a| a.mcp.user.and_then(|l| l.env)))
            .chain(with_skills().filter_map(|a| a.skills.as_ref().and_then(|s| s.user.env)))
            .chain(["XDG_STATE_HOME"])
            .filter_map(|name| std::env::var_os(name).map(|v| (name.to_owned(), v)))
            .collect();
        vars.sort();
        vars.dedup();
        Ok(Self {
            home,
            cwd: std::env::current_dir()?,
            os: Os::current(),
            vars,
        })
    }

    /// The variable `name`, unless it is unset or empty.
    pub fn var(&self, name: &str) -> Option<&OsString> {
        self.vars
            .iter()
            .find(|(n, v)| n == name && !v.is_empty())
            .map(|(_, v)| v)
    }

    /// The program `name` on `PATH`, if it is there.
    pub fn which(&self, name: &str) -> Option<PathBuf> {
        let path = self.var("PATH")?;
        std::env::split_paths(path)
            .filter(|dir| dir.is_absolute())
            .map(|dir| dir.join(name))
            .find(|candidate| is_executable(candidate))
    }

    fn base(&self, base: Base) -> PathBuf {
        match (base, self.os) {
            (Base::Home, _) => self.home.clone(),
            (Base::AppConfig, Os::MacOs) => self.home.join("Library/Application Support"),
            (Base::AppConfig, Os::Unix) => self
                .var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| self.home.join(".config")),
        }
    }

    /// Where `location` is on this host.
    pub fn resolve(&self, location: &Location) -> PathBuf {
        let dir = match location.env.and_then(|name| self.var(name)) {
            Some(dir) => PathBuf::from(dir),
            None => self.base(location.base).join(location.dir),
        };
        dir.join(location.file)
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

impl Agent {
    /// The file the agent reads its servers from at `scope` on `host`, or
    /// why there is none.
    pub fn file(&self, scope: Scope, host: &Host) -> anyhow::Result<PathBuf> {
        match scope {
            Scope::User => self.mcp.user.map(|l| host.resolve(&l)),
            Scope::Project => self.mcp.project.map(|p| host.cwd.join(p)),
        }
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} has no {} scope: use --scope {}",
                self.name,
                scope.as_str(),
                match scope {
                    Scope::User => Scope::Project,
                    Scope::Project => Scope::User,
                }
                .as_str()
            )
        })
    }

    /// The directory the agent reads skills from at `scope` on `host`, or
    /// why there is none.
    pub fn skills_dir(&self, scope: Scope, host: &Host) -> anyhow::Result<PathBuf> {
        let skills = self
            .skills
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("{} doesn't read agent skills", self.name))?;
        Ok(match scope {
            Scope::User => host.resolve(&skills.user),
            Scope::Project => host.cwd.join(skills.project),
        })
    }

    /// The agent's command for `scope`, when it has one that covers it.
    pub fn command(&self, scope: Scope) -> Option<AgentCommand> {
        self.mcp.command.filter(|c| c.covers(scope))
    }

    /// Whether the agent looks installed on `host`: one of its programs is
    /// on `PATH`, or its user file, or the directory that would hold it,
    /// exists.
    pub fn detected(&self, host: &Host) -> bool {
        self.programs.iter().any(|p| host.which(p).is_some())
            || self.mcp.user.is_some_and(|l| {
                let file = host.resolve(&l);
                // The home directory itself proves nothing.
                file.exists() || file.parent().is_some_and(|d| d != host.home && d.is_dir())
            })
    }
}

/// The agents `ids` names; without any, on a terminal, those the user picks
/// of the ones in `offered` found on this machine, each shown with `place`,
/// what the command changes for it. `question` asks which.
pub(crate) fn choose(
    host: &Host,
    ids: &[String],
    offered: &[&'static Agent],
    place: impl Fn(&Agent) -> String,
    question: &str,
) -> anyhow::Result<Vec<&'static Agent>> {
    if !ids.is_empty() {
        let mut chosen: Vec<&'static Agent> = Vec::new();
        for id in ids {
            let agent = agent(id).expect("clap checked the value");
            if !chosen.iter().any(|a| a.id == agent.id) {
                chosen.push(agent);
            }
        }
        return Ok(chosen);
    }
    let names = offered.iter().map(|a| a.id).collect::<Vec<_>>().join(", ");
    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        bail!("name the agent with --agent ({names}): without a terminal there is no one to ask");
    }
    let found: Vec<&'static Agent> = offered
        .iter()
        .copied()
        .filter(|a| a.detected(host))
        .collect();
    if found.is_empty() {
        bail!("found none of the agents on this machine: name one with --agent ({names})");
    }
    let mut stderr = std::io::stderr();
    writeln!(stderr, "Agents found on this machine:")?;
    for (i, agent) in found.iter().enumerate() {
        writeln!(
            stderr,
            "  {}. {} ({})  {}",
            i + 1,
            agent.name,
            agent.id,
            place(agent)
        )?;
    }
    write!(
        stderr,
        "{question} Numbers separated by commas, Enter for all: "
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

/// How a location is written in the help (`code` false) and the README
/// (`code` true): `~/.codex/config.toml, or $CODEX_HOME/config.toml`.
fn describe_location(location: &Location, code: bool) -> String {
    let path = |base: &str| {
        let path = if location.dir.is_empty() {
            format!("{base}/{}", location.file)
        } else {
            format!("{base}/{}/{}", location.dir, location.file)
        };
        if code {
            format!("`{path}`")
        } else {
            path
        }
    };
    let default = match location.base {
        Base::Home => path("~"),
        Base::AppConfig => format!(
            "{} (macOS), {} (Linux)",
            path("~/Library/Application Support"),
            path("~/.config")
        ),
    };
    match location.env {
        Some(env) => {
            let moved = format!("${env}/{}", location.file);
            let moved = if code { format!("`{moved}`") } else { moved };
            format!("{default}, or {moved}")
        }
        None => default,
    }
}

/// The mechanism, as the help and the README say it.
fn describe_mechanism(agent: &Agent) -> String {
    match agent.mcp.command {
        Some(command) => {
            let scopes = if command.covers(Scope::Project) {
                ""
            } else {
                " (user scope)"
            };
            format!(
                "`{}`{scopes} when `{}` is on PATH, else the file",
                command.describe("add"),
                command.program()
            )
        }
        None => "the file".to_owned(),
    }
}

/// The table as Markdown, as the README has it (a test checks it does).
#[cfg(test)]
pub(crate) fn markdown_table() -> String {
    let mut out = String::from(
        "| `--agent` | Agent | User scope | Project scope | Format | Added with |\n\
         |---|---|---|---|---|---|\n",
    );
    for agent in AGENTS {
        let user = agent
            .mcp
            .user
            .as_ref()
            .map(|l| describe_location(l, true))
            .unwrap_or_else(|| "–".to_owned());
        let project = agent
            .mcp
            .project
            .map(|p| format!("`{p}`"))
            .unwrap_or_else(|| "–".to_owned());
        let mut format = format!(
            "{}, `{}`",
            agent.mcp.format.syntax.describe(),
            agent.mcp.format.servers
        );
        if agent.gui {
            format.push_str("; absolute path of `origin`");
        }
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} |\n",
            agent.id,
            agent.name,
            user,
            project,
            format,
            describe_mechanism(agent)
        ));
    }
    out
}

/// The table as the long help of `origin mcp install` lists it.
pub(crate) fn help_table() -> String {
    let mut out = String::from("Agents:\n");
    for agent in AGENTS {
        out.push_str(&format!("\n  {} ({})\n", agent.id, agent.name));
        if let Some(user) = &agent.mcp.user {
            out.push_str(&format!(
                "    user:     {}\n",
                describe_location(user, false)
            ));
        }
        if let Some(project) = agent.mcp.project {
            out.push_str(&format!("    project:  {project}\n"));
        }
        out.push_str(&format!(
            "    format:   {}, under `{}`\n    added by: {}\n",
            agent.mcp.format.syntax.describe(),
            agent.mcp.format.servers,
            describe_mechanism(agent).replace('`', "")
        ));
        if agent.gui {
            out.push_str("    The entry names `origin` by its absolute path.\n");
        }
    }
    out
}

/// The agents that share each skills directory of the project scope, in
/// the table's order: `.agents/skills` is read by several.
fn sharing(project: &str) -> Vec<&'static str> {
    with_skills()
        .filter(|a| a.skills.as_ref().is_some_and(|s| s.project == project))
        .map(|a| a.id)
        .collect()
}

/// The skill directories as Markdown, as the README has them (a test checks
/// it does).
#[cfg(test)]
pub(crate) fn skills_markdown_table() -> String {
    let mut out = String::from(
        "| `--agent` | Agent | User scope | Project scope |\n\
         |---|---|---|---|\n",
    );
    for agent in with_skills() {
        let skills = agent.skills.as_ref().expect("with skills");
        let mut project = format!("`{}`", skills.project);
        if sharing(skills.project).len() > 1 {
            project.push_str(", shared");
        }
        out.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            agent.id,
            agent.name,
            describe_location(&skills.user, true),
            project
        ));
    }
    out
}

/// The skill directories as the long help of `origin skill` lists them.
pub(crate) fn skills_help_table() -> String {
    let mut out = String::from("Agents, and the directories they read skills from:\n");
    for agent in with_skills() {
        let skills = agent.skills.as_ref().expect("with skills");
        out.push_str(&format!(
            "\n  {} ({})\n    user:     {}\n    project:  {}\n",
            agent.id,
            agent.name,
            describe_location(&skills.user, false),
            skills.project
        ));
        let others: Vec<&str> = sharing(skills.project)
            .into_iter()
            .filter(|id| *id != agent.id)
            .collect();
        if !others.is_empty() {
            out.push_str(&format!(
                "              shared with {}\n",
                others.join(", ")
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(os: Os, vars: &[(&str, &str)]) -> Host {
        Host {
            home: PathBuf::from("/home/u"),
            cwd: PathBuf::from("/work/p"),
            os,
            vars: vars
                .iter()
                .map(|(n, v)| ((*n).to_owned(), OsString::from(v)))
                .collect(),
        }
    }

    fn user_file(id: &str, host: &Host) -> PathBuf {
        agent(id).unwrap().file(Scope::User, host).unwrap()
    }

    #[test]
    fn the_ids_are_unique_and_every_agent_has_a_scope() {
        let mut ids: Vec<_> = AGENTS.iter().map(|a| a.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), AGENTS.len());
        for agent in AGENTS {
            assert!(agent.mcp.user.is_some() || agent.mcp.project.is_some());
        }
    }

    #[test]
    fn the_user_files_resolve_per_system() {
        let linux = host(Os::Unix, &[]);
        let mac = host(Os::MacOs, &[]);
        assert_eq!(
            user_file("claude-code", &linux),
            Path::new("/home/u/.claude.json")
        );
        assert_eq!(
            user_file("codex", &linux),
            Path::new("/home/u/.codex/config.toml")
        );
        assert_eq!(
            user_file("cursor", &mac),
            Path::new("/home/u/.cursor/mcp.json")
        );
        assert_eq!(
            user_file("gemini-cli", &linux),
            Path::new("/home/u/.gemini/settings.json")
        );
        assert_eq!(
            user_file("github-copilot", &mac),
            Path::new("/home/u/.copilot/mcp-config.json")
        );
        assert_eq!(
            user_file("claude-desktop", &linux),
            Path::new("/home/u/.config/Claude/claude_desktop_config.json")
        );
        assert_eq!(
            user_file("claude-desktop", &mac),
            Path::new("/home/u/Library/Application Support/Claude/claude_desktop_config.json")
        );
    }

    #[test]
    fn a_variable_moves_a_user_file() {
        let moved = host(
            Os::Unix,
            &[
                ("CODEX_HOME", "/opt/codex"),
                ("CLAUDE_CONFIG_DIR", "/opt/claude"),
                ("COPILOT_HOME", "/opt/copilot"),
                ("XDG_CONFIG_HOME", "/xdg"),
            ],
        );
        assert_eq!(
            user_file("codex", &moved),
            Path::new("/opt/codex/config.toml")
        );
        assert_eq!(
            user_file("claude-code", &moved),
            Path::new("/opt/claude/.claude.json")
        );
        assert_eq!(
            user_file("github-copilot", &moved),
            Path::new("/opt/copilot/mcp-config.json")
        );
        assert_eq!(
            user_file("claude-desktop", &moved),
            Path::new("/xdg/Claude/claude_desktop_config.json")
        );
        // macOS doesn't read XDG_CONFIG_HOME.
        let mac = Host {
            os: Os::MacOs,
            ..moved
        };
        assert_eq!(
            user_file("claude-desktop", &mac),
            Path::new("/home/u/Library/Application Support/Claude/claude_desktop_config.json")
        );
    }

    #[test]
    fn the_project_files_are_under_the_current_directory() {
        let host = host(Os::Unix, &[]);
        let file = |id| agent(id).unwrap().file(Scope::Project, &host).unwrap();
        assert_eq!(file("claude-code"), Path::new("/work/p/.mcp.json"));
        assert_eq!(file("github-copilot"), Path::new("/work/p/.mcp.json"));
        assert_eq!(file("codex"), Path::new("/work/p/.codex/config.toml"));
        let err = agent("claude-desktop")
            .unwrap()
            .file(Scope::Project, &host)
            .unwrap_err();
        assert!(err.to_string().contains("no project scope"), "{err}");
    }

    #[test]
    fn the_commands_put_the_servers_arguments_after_a_separator() {
        let args = vec![
            "mcp".to_owned(),
            "serve".to_owned(),
            "--read-only".to_owned(),
        ];
        assert_eq!(
            AgentCommand::Claude
                .add(Scope::Project, "oc", "origin", &args)
                .join(" "),
            "mcp add --scope project oc -- origin mcp serve --read-only"
        );
        assert_eq!(
            AgentCommand::Codex
                .add(Scope::User, "oc", "origin", &args)
                .join(" "),
            "mcp add oc -- origin mcp serve --read-only"
        );
        assert_eq!(
            AgentCommand::Gemini
                .add(Scope::User, "oc", "origin", &args)
                .join(" "),
            "mcp add --scope user oc origin -- mcp serve --read-only"
        );
        assert!(!AgentCommand::Codex.covers(Scope::Project));
    }

    /// The README's part between `<!-- {marker}: generated from … -->` and
    /// `<!-- /{marker} -->` is `table`; with UPDATE_GOLDEN set, it is made so.
    fn readme_has(marker: &str, table: &str) {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../README.md");
        let readme = std::fs::read_to_string(path).unwrap();
        let start = format!("<!-- {marker}: generated from crates/cli/src/handler/agents.rs -->\n");
        let at = readme.find(&start).expect("the README marks the table") + start.len();
        let end = readme[at..]
            .find(&format!("<!-- /{marker} -->"))
            .expect("and its end")
            + at;
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            let updated = format!("{}{}{}", &readme[..at], table, &readme[end..]);
            std::fs::write(path, updated).unwrap();
            return;
        }
        assert_eq!(
            &readme[at..end],
            table,
            "the README's table {marker} isn't the one in agents.rs: copy it from there"
        );
    }

    /// The README's tables of agents are these.
    #[test]
    fn the_readme_lists_the_tables() {
        readme_has("agents", &markdown_table());
        readme_has("skill-agents", &skills_markdown_table());
    }

    #[test]
    fn the_skill_directories_resolve_per_scope() {
        let linux = host(Os::Unix, &[]);
        let dir =
            |id: &str, scope, host: &Host| agent(id).unwrap().skills_dir(scope, host).unwrap();
        assert_eq!(
            dir("claude-code", Scope::User, &linux),
            Path::new("/home/u/.claude/skills")
        );
        assert_eq!(
            dir("codex", Scope::User, &linux),
            Path::new("/home/u/.agents/skills")
        );
        assert_eq!(
            dir("cursor", Scope::User, &linux),
            Path::new("/home/u/.cursor/skills")
        );
        assert_eq!(
            dir("gemini-cli", Scope::User, &linux),
            Path::new("/home/u/.gemini/skills")
        );
        assert_eq!(
            dir("github-copilot", Scope::User, &linux),
            Path::new("/home/u/.copilot/skills")
        );
        assert_eq!(
            dir("claude-code", Scope::Project, &linux),
            Path::new("/work/p/.claude/skills")
        );
        for id in ["codex", "cursor", "gemini-cli", "github-copilot"] {
            assert_eq!(
                dir(id, Scope::Project, &linux),
                Path::new("/work/p/.agents/skills"),
                "{id}"
            );
        }
        let moved = host(
            Os::Unix,
            &[
                ("CLAUDE_CONFIG_DIR", "/opt/claude"),
                ("COPILOT_HOME", "/opt/copilot"),
            ],
        );
        assert_eq!(
            dir("claude-code", Scope::User, &moved),
            Path::new("/opt/claude/skills")
        );
        assert_eq!(
            dir("github-copilot", Scope::User, &moved),
            Path::new("/opt/copilot/skills")
        );
        let err = agent("claude-desktop")
            .unwrap()
            .skills_dir(Scope::User, &linux)
            .unwrap_err();
        assert!(
            err.to_string().contains("doesn't read agent skills"),
            "{err}"
        );
    }

    #[test]
    fn an_answer_picks_agents_by_number() {
        assert_eq!(choice("\n", 3).unwrap(), vec![0, 1, 2]);
        assert_eq!(choice("all\n", 2).unwrap(), vec![0, 1]);
        assert_eq!(choice("2, 1 2\n", 3).unwrap(), vec![1, 0]);
        assert!(choice("4", 3).is_err());
        assert!(choice("0", 3).is_err());
        assert!(choice("claude", 3).is_err());
    }
}
