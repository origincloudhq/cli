//! `origin skill` subcommand group: Origin Cloud's agent skills, embedded
//! from origincloudhq/skills, installed into the coding agents' skill
//! directories and kept current.
//!
//! `commands.rs` declares the clap surface (`SkillCommands`); `embedded.rs`
//! holds the skills; `install.rs` installs, lists and updates them in the
//! directories of the table in `agents.rs`; `marker.rs` is how an installed
//! copy says its version; `state.rs` remembers where the copies are, and
//! points out outdated ones after an upgrade.

mod commands;
mod embedded;
mod install;
mod marker;
mod state;

pub(crate) use commands::SkillCommands;
pub(crate) use install::run_skill_command;
pub(crate) use state::notice;
