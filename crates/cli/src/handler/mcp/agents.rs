//! The coding agents `origin mcp install` registers the MCP server with, in
//! one table, [`AGENTS`]: what each is called, how to tell it is installed,
//! and, for each scope, the file its MCP servers are configured in, that
//! file's format, and the agent's own command for adding one.
//!
//! `--agent`'s values and the help of `origin mcp install`, the README's
//! table of agents (a test checks it is this one) and the tests all read
//! it, so adding an agent is a row here, and a format adapter in
//! `config_file.rs` when it brings a new format. Each agent's capabilities
//! are a field of their own (`mcp`), so the skills an agent reads, and the
//! directory it reads them from, can be another without the rest changing.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Where an agent's MCP configuration applies: to every project of the
/// user, or to the project in the current directory, in a file that is
/// usually committed with it.
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

/// A file in a user's directory: `dir`/`file` under `base`, or under the
/// directory the variable `env` names when it is set.
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
    /// and `PATH`.
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
            .filter_map(|name| std::env::var_os(name).map(|v| (name.to_owned(), v)))
            .collect();
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

    /// The README's table of agents is this one.
    #[test]
    fn the_readme_lists_the_table() {
        let readme = include_str!("../../../../../README.md");
        let start = "<!-- agents: generated from crates/cli/src/handler/mcp/agents.rs -->\n";
        let at = readme.find(start).expect("the README marks the table") + start.len();
        let end = readme[at..].find("<!-- /agents -->").expect("and its end") + at;
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../README.md");
            let updated = format!("{}{}{}", &readme[..at], markdown_table(), &readme[end..]);
            std::fs::write(path, updated).unwrap();
            return;
        }
        assert_eq!(
            &readme[at..end],
            markdown_table(),
            "the README's table of agents isn't the one in agents.rs: copy markdown_table()'s"
        );
    }
}
