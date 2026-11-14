use std::path::PathBuf;

use clap::builder::PossibleValuesParser;
use clap::{Args, Subcommand};

use super::embedded;
use crate::handler::agents::{self, Scope};

#[derive(Subcommand, Debug)]
pub(crate) enum SkillCommands {
    /// Install Origin Cloud's agent skills for coding agents.
    ///
    /// Writes each skill's directory, `SKILL.md` and its references, into
    /// the directory the agent reads skills from: `origin` when no skill is
    /// named, every one with `--all`. The skills are those of the
    /// origincloudhq/skills commit this `origin` was built with, so
    /// installing needs no network access, and the `origin` skill describes
    /// this version's commands. At the project scope, the agents that read
    /// `.agents/skills` share one copy there.
    ///
    /// The `SKILL.md` it writes says in its `metadata` which version it is
    /// (`origin-skills-revision`, `origin-skills-digest`). A skill's
    /// directory that `origin` didn't install, such as one `gh skill
    /// install` did, or one changed since, is left as it is, and nothing is
    /// installed, unless `--force` is given; the agent's other skills are
    /// never touched. Running it again changes nothing; after an upgrade of
    /// `origin`, it updates the skills, as `origin skill update` does. On a
    /// terminal, without `--agent` or `--dir`, it offers the agents it finds
    /// on this machine.
    #[command(after_long_help = agents::skills_help_table())]
    Install(InstallArgs),

    /// List the agent skills this `origin` carries, and where which version
    /// is installed.
    ///
    /// For each skill, its version (the origincloudhq/skills commit), and
    /// each copy in the agents' skill directories, of the user and of the
    /// project in the current directory, and wherever else `origin skill
    /// install` installed one: its version, and whether it is `current`,
    /// `outdated`, `modified` since it was installed, or `unmanaged`, a
    /// copy `origin` didn't install.
    #[command(after_long_help = agents::skills_help_table())]
    List(ListArgs),

    /// Replace the installed agent skills with the version this `origin`
    /// carries.
    ///
    /// Updates every outdated copy `origin skill list` lists, or those of
    /// the skills named: in the agents' skill directories, of the user and
    /// of the project in the current directory, and wherever else `origin
    /// skill install` installed one. A copy changed since it was installed,
    /// or one `origin` didn't install, is skipped; `origin skill install
    /// --force` replaces it.
    #[command(after_long_help = agents::skills_help_table())]
    Update(UpdateArgs),
}

/// The agents' directories, or another.
#[derive(Args, Debug, Clone, Default)]
pub(crate) struct Where {
    /// The coding agents, comma-separated: their skill directories.
    #[arg(
        long = "agent",
        value_name = "AGENT",
        value_delimiter = ',',
        value_parser = PossibleValuesParser::new(agents::skill_possible_values()),
        conflicts_with = "dir"
    )]
    pub agents: Vec<String>,
    /// `user`, the agent's own skill directory, for every project; or
    /// `project`, the project in the current directory, in a directory that
    /// is usually committed with it.
    #[arg(long, value_parser = agents::skill_scope_parser(), conflicts_with = "dir")]
    pub scope: Option<Scope>,
    /// A skills directory of your own instead of an agent's: each skill is
    /// a directory in it.
    #[arg(long, value_name = "PATH")]
    pub dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub(crate) struct InstallArgs {
    /// The skills to install; `origin` when none is named.
    #[arg(
        value_name = "NAME",
        value_parser = PossibleValuesParser::new(embedded::names()),
        conflicts_with = "all"
    )]
    pub names: Vec<String>,
    /// Install every skill this `origin` carries.
    #[arg(long)]
    pub all: bool,
    /// The agents, the scope (`user` unless given), or a directory; on a
    /// terminal without `--agent` or `--dir`, the agents found on this
    /// machine are offered.
    #[command(flatten)]
    pub place: Where,
    /// Replace a skill's directory `origin` didn't install, or one changed
    /// since it did.
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub(crate) struct ListArgs {
    /// Only these agents, this scope (both unless given), or this
    /// directory.
    #[command(flatten)]
    pub place: Where,
}

#[derive(Args, Debug)]
pub(crate) struct UpdateArgs {
    /// The skills to update; every one when none is named.
    #[arg(value_name = "NAME", value_parser = PossibleValuesParser::new(embedded::names()))]
    pub names: Vec<String>,
    /// Only these agents, this scope (both unless given), or this
    /// directory.
    #[command(flatten)]
    pub place: Where,
}
