//! `origin skill install`, `list` and `update`: the embedded skills
//! (`embedded.rs`) written into the skill directories of the coding agents
//! of the table in `agents.rs`, or into another directory.
//!
//! A skill is a directory of its own, `{skills directory}/{name}/`, and
//! nothing else in the skills directory is touched. What is at that path
//! decides what happens to it ([`inspect`]): a copy `origin` wrote, and no
//! one changed since, as its marker (`marker.rs`) proves, is updated when it
//! is of another version; anything else is someone else's, and only
//! `--force` replaces it. A new version is written beside the old one and
//! renamed over it, so that an agent never reads half of each.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::Serialize;
use serde_json::Value;

use super::commands::{InstallArgs, ListArgs, SkillCommands, UpdateArgs, Where};
use super::embedded::{self, short, Skill, REVISION};
use super::marker::{self, Marker};
use super::state::{self, Record};
use crate::handler::agents::{self, Agent, Host, Scope};
use crate::handler::files::{create, resolve_links};
use crate::output::{Listing, SerializableOutput, SerializationError, Shape};

/// Runs `origin skill install`, `list` or `update`.
pub(crate) fn run_skill_command(cmd: SkillCommands) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let host = Host::current()?;
    match cmd {
        SkillCommands::Install(args) => install(&host, args),
        SkillCommands::List(args) => list(&host, args),
        SkillCommands::Update(args) => update(&host, args),
    }
}

/// A skills directory, and the agents that read it at a scope; neither for
/// a directory given with `--dir`.
#[derive(Debug, Clone)]
struct Place {
    dir: PathBuf,
    agents: Vec<String>,
    scope: Option<Scope>,
}

impl Place {
    fn record(&self, name: &str) -> Record {
        Record {
            path: self.dir.join(name),
            agents: self.agents.clone(),
            scope: self.scope.map(|s| s.as_str().to_owned()),
        }
    }
}

/// The skills directories of `agents` at `scopes`, one place for each, its
/// agents together where they share it.
fn places_of(host: &Host, agents: &[&Agent], scopes: &[Scope]) -> anyhow::Result<Vec<Place>> {
    let mut places: Vec<Place> = Vec::new();
    for &scope in scopes {
        for agent in agents {
            let dir = agent.skills_dir(scope, host)?;
            match places.iter_mut().find(|p| p.dir == dir) {
                Some(place) => place.agents.push(agent.id.to_owned()),
                None => places.push(Place {
                    dir,
                    agents: vec![agent.id.to_owned()],
                    scope: Some(scope),
                }),
            }
        }
    }
    Ok(places)
}

/// The places `list` and `update` look in: `--dir`; or the skill
/// directories of the agents (`--agent`, else every one) at the scope
/// (`--scope`, else both), and those `origin skill install` wrote to
/// elsewhere that match.
fn known_places(host: &Host, place: &Where) -> anyhow::Result<Vec<Place>> {
    if let Some(dir) = &place.dir {
        return Ok(vec![Place {
            dir: absolute(dir)?,
            agents: Vec::new(),
            scope: None,
        }]);
    }
    let agents: Vec<&Agent> = if place.agents.is_empty() {
        agents::with_skills().collect()
    } else {
        place
            .agents
            .iter()
            .map(|id| agents::agent(id).expect("clap checked the value"))
            .collect()
    };
    let scopes = match place.scope {
        Some(scope) => vec![scope],
        None => vec![Scope::User, Scope::Project],
    };
    let mut places = places_of(host, &agents, &scopes)?;
    for record in state::load(host).installed {
        let Some(dir) = record.path.parent() else {
            continue;
        };
        let agent_matches =
            place.agents.is_empty() || record.agents.iter().any(|a| place.agents.contains(a));
        let scope_matches = place
            .scope
            .is_none_or(|s| record.scope.as_deref() == Some(s.as_str()));
        if agent_matches && scope_matches && !places.iter().any(|p| p.dir == dir) {
            places.push(Place {
                dir: dir.to_path_buf(),
                agents: record.agents.clone(),
                scope: match record.scope.as_deref() {
                    Some("user") => Some(Scope::User),
                    Some("project") => Some(Scope::Project),
                    _ => None,
                },
            });
        }
    }
    Ok(places)
}

fn absolute(dir: &Path) -> anyhow::Result<PathBuf> {
    std::path::absolute(dir).with_context(|| format!("--dir {}", dir.display()))
}

/// What is at a skill's path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Copy {
    /// A copy `origin` wrote, of the version this `origin` carries.
    Current(Marker),
    /// A copy `origin` wrote, of another version.
    Outdated(Marker),
    /// A copy `origin` wrote, changed since.
    Modified(Marker),
    /// Anything else: a skill `origin` didn't install, or not a skill.
    Unmanaged,
}

/// What is at `target`, the directory of `skill` in a skills directory:
/// nothing (`None`), or a copy of it, which is `origin`'s while its files,
/// with the marker taken out of its `SKILL.md`, have the digest the marker
/// names.
pub(super) fn inspect(target: &Path, skill: &Skill) -> anyhow::Result<Option<Copy>> {
    match std::fs::symlink_metadata(target) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("reading {}", target.display())),
        Ok(_) => {}
    }
    let real = resolve_links(target)?;
    let meta = match std::fs::metadata(&real) {
        // A link to nothing: writing it creates its target.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("reading {}", real.display())),
        Ok(meta) => meta,
    };
    if !meta.is_dir() {
        return Ok(Some(Copy::Unmanaged));
    }
    let text = match std::fs::read(real.join("SKILL.md")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Some(Copy::Unmanaged)),
        Err(e) => return Err(e).with_context(|| format!("reading {}/SKILL.md", real.display())),
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => return Ok(Some(Copy::Unmanaged)),
        },
    };
    let Some(found) = marker::read(&text) else {
        return Ok(Some(Copy::Unmanaged));
    };
    let mut files = Vec::new();
    if !regular_files(&real, &real, &mut files)? {
        return Ok(Some(Copy::Modified(found)));
    }
    let stripped = marker::strip(&text);
    let digest = marker::digest(files.iter().map(|(path, content)| {
        let content: &[u8] = if path == "SKILL.md" {
            stripped.as_bytes()
        } else {
            content
        };
        (path.as_str(), content)
    }));
    Ok(Some(if digest != found.digest {
        Copy::Modified(found)
    } else if found.digest == skill.digest() {
        Copy::Current(found)
    } else {
        Copy::Outdated(found)
    }))
}

/// Every file under `dir`, by its path relative to `root`; false when
/// something in it is neither a regular file nor a directory, which
/// `origin` never writes.
fn regular_files(
    root: &Path,
    dir: &Path,
    files: &mut Vec<(String, Vec<u8>)>,
) -> anyhow::Result<bool> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        let kind = std::fs::symlink_metadata(&path)?.file_type();
        if kind.is_dir() {
            if !regular_files(root, &path, files)? {
                return Ok(false);
            }
        } else if kind.is_file() {
            let Some(relative) = path
                .strip_prefix(root)
                .ok()
                .and_then(|p| p.to_str())
                .map(|p| p.replace(std::path::MAIN_SEPARATOR, "/"))
            else {
                return Ok(false);
            };
            let content =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            files.push((relative, content));
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Writes `skill` to `target`, with its marker, replacing what is there:
/// into a directory beside it first, which is then renamed over it. A
/// `target` that is a symbolic link stays one, and its target is written;
/// a file that is replaced keeps its permissions, and the directory its.
fn write(target: &Path, skill: &Skill) -> anyhow::Result<()> {
    let real = resolve_links(target)?;
    let parent = real
        .parent()
        .with_context(|| format!("{} has no directory", real.display()))?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    let name = real
        .file_name()
        .with_context(|| format!("{} has no name", real.display()))?
        .to_string_lossy()
        .into_owned();
    // Hidden, and gone at the end: no agent takes them for skills.
    let temporary = parent.join(format!(".{name}.origin.tmp"));
    let old = parent.join(format!(".{name}.origin.old"));
    remove(&temporary)?;
    remove(&old)?;

    let existing = std::fs::metadata(&real).ok().filter(|m| m.is_dir());
    std::fs::create_dir(&temporary).with_context(|| format!("creating {}", temporary.display()))?;
    if let Some(meta) = &existing {
        std::fs::set_permissions(&temporary, meta.permissions())?;
    }
    let digest = skill.digest();
    for (path, content) in &skill.files {
        let file = temporary.join(path);
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let permissions = existing
            .as_ref()
            .and_then(|_| std::fs::metadata(real.join(path)).ok())
            .filter(|m| m.is_file())
            .map(|m| m.permissions());
        let written = if *path == "SKILL.md" {
            let text = std::str::from_utf8(content).context("the skill's SKILL.md isn't UTF-8")?;
            create(
                &file,
                marker::mark(text, REVISION, &digest)?,
                permissions.as_ref(),
            )
        } else {
            create(&file, content, permissions.as_ref())
        };
        written.with_context(|| format!("writing {}", file.display()))?;
    }

    let replacing = std::fs::symlink_metadata(&real).is_ok();
    if replacing {
        std::fs::rename(&real, &old).with_context(|| format!("moving {} aside", real.display()))?;
    }
    if let Err(e) = std::fs::rename(&temporary, &real) {
        if replacing {
            let _ = std::fs::rename(&old, &real);
        }
        let _ = remove(&temporary);
        return Err(e).with_context(|| format!("writing {}", real.display()));
    }
    remove(&old)
}

/// Removes `path`, a directory or a file, if it is there.
fn remove(path: &Path) -> anyhow::Result<()> {
    let result = match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => Err(e),
        Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
    };
    result.with_context(|| format!("removing {}", path.display()))
}

/// The skills named, in order and once each.
fn chosen(names: &[String]) -> Vec<Skill> {
    let mut skills: Vec<Skill> = Vec::new();
    for name in names {
        if !skills.iter().any(|s| s.name == name) {
            skills.push(embedded::skill(name).expect("clap checked the name"));
        }
    }
    skills
}

/// Why a copy isn't `origin`'s to replace.
fn whose(copy: &Copy) -> &'static str {
    match copy {
        Copy::Modified(_) => "was changed since `origin` installed it",
        _ => {
            "wasn't installed by `origin` (another tool's, such as `gh skill install`, or your own)"
        }
    }
}

/// What install or update did with one skill in one place.
#[derive(Debug, Serialize)]
struct Outcome {
    name: &'static str,
    agents: Vec<String>,
    scope: Option<&'static str>,
    path: String,
    /// `added`, `updated`, `replaced`, `unchanged` or `skipped`.
    result: &'static str,
    /// The version there now: the origincloudhq/skills commit.
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

fn outcome(place: &Place, skill: &Skill, result: &'static str, version: Option<&str>) -> Outcome {
    Outcome {
        name: skill.name,
        agents: place.agents.clone(),
        scope: place.scope.map(Scope::as_str),
        path: place.dir.join(skill.name).display().to_string(),
        result,
        version: version.map(|v| short(v).to_owned()),
        reason: None,
    }
}

fn install(host: &Host, args: InstallArgs) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let names: Vec<String> = if args.all {
        embedded::names().into_iter().map(str::to_owned).collect()
    } else if args.names.is_empty() {
        vec![embedded::DEFAULT.to_owned()]
    } else {
        args.names.clone()
    };
    let skills = chosen(&names);
    let places = match &args.place.dir {
        Some(dir) => vec![Place {
            dir: absolute(dir)?,
            agents: Vec::new(),
            scope: None,
        }],
        None => {
            let scope = args.place.scope.unwrap_or(Scope::User);
            let offered: Vec<&'static Agent> = agents::with_skills().collect();
            let chosen = agents::choose(
                host,
                &args.place.agents,
                &offered,
                |agent| {
                    agent
                        .skills_dir(scope, host)
                        .map(|d| d.display().to_string())
                        .unwrap_or_default()
                },
                &format!(
                    "Which to install the skills for, at the {} scope?",
                    scope.as_str()
                ),
            )?;
            places_of(host, &chosen, &[scope])?
        }
    };

    // Everything is checked before anything is written: a refusal leaves
    // every directory as it was.
    let mut plan = Vec::new();
    let mut refused = Vec::new();
    for place in &places {
        for skill in &skills {
            let target = place.dir.join(skill.name);
            let (result, version) = match inspect(&target, skill)? {
                None => ("added", REVISION.to_owned()),
                // The copy's own version, which has this content.
                Some(Copy::Current(m)) => ("unchanged", m.revision),
                Some(Copy::Outdated(_)) => ("updated", REVISION.to_owned()),
                Some(_) if args.force => ("replaced", REVISION.to_owned()),
                Some(copy) => {
                    refused.push(format!("{} {}", target.display(), whose(&copy)));
                    continue;
                }
            };
            plan.push((place, skill, result, version));
        }
    }
    if !refused.is_empty() {
        bail!(
            "{}: left as it is, and nothing was installed; --force replaces it",
            refused.join("; ")
        );
    }

    let mut outcomes = Vec::new();
    let mut records = Vec::new();
    for (place, skill, result, version) in plan {
        if result != "unchanged" {
            write(&place.dir.join(skill.name), skill).map_err(|e| done_before(e, &outcomes))?;
        }
        outcomes.push(outcome(place, skill, result, Some(&version)));
        records.push(place.record(skill.name));
    }
    remember(host, records);
    Ok(Box::new(Outcomes(outcomes)))
}

/// `error`, with the skills written before it, which it would otherwise
/// leave unsaid.
fn done_before(error: anyhow::Error, done: &[Outcome]) -> anyhow::Error {
    if done.is_empty() {
        return error;
    }
    let done: Vec<String> = done
        .iter()
        .map(|o| format!("{} {}", o.path, o.result))
        .collect();
    anyhow::anyhow!("{error:#} (before it: {})", done.join(", "))
}

/// Notes where the skills are, for `list`, `update` and the notice of
/// outdated ones; a failure to is said, and changes nothing else.
fn remember(host: &Host, records: Vec<Record>) {
    if let Err(e) = state::remember(host, records) {
        eprintln!("warning: couldn't note where the skills are installed: {e:#}");
    }
}

fn update(host: &Host, args: UpdateArgs) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let skills = if args.names.is_empty() {
        embedded::all()
    } else {
        chosen(&args.names)
    };
    let mut outcomes = Vec::new();
    let mut records = Vec::new();
    for place in known_places(host, &args.place)? {
        for skill in &skills {
            let target = place.dir.join(skill.name);
            let Some(copy) = inspect(&target, skill)? else {
                continue;
            };
            let outcome = match &copy {
                Copy::Current(m) => outcome(&place, skill, "unchanged", Some(&m.revision)),
                Copy::Outdated(_) => {
                    write(&target, skill).map_err(|e| done_before(e, &outcomes))?;
                    outcome(&place, skill, "updated", Some(REVISION))
                }
                Copy::Modified(m) => Outcome {
                    reason: Some(whose(&copy).to_owned()),
                    ..outcome(&place, skill, "skipped", Some(&m.revision))
                },
                Copy::Unmanaged => Outcome {
                    reason: Some(whose(&copy).to_owned()),
                    ..outcome(&place, skill, "skipped", None)
                },
            };
            if !matches!(copy, Copy::Unmanaged) {
                records.push(place.record(skill.name));
            }
            outcomes.push(outcome);
        }
    }
    remember(host, records);
    Ok(Box::new(Outcomes(outcomes)))
}

/// One row of `origin skill list`: a skill, and one copy of it, or none.
#[derive(Debug, Serialize)]
struct Row {
    name: &'static str,
    /// The version this `origin` carries: the origincloudhq/skills commit.
    version: String,
    /// `current`, `outdated`, `modified`, `unmanaged`, or `not-installed`.
    status: &'static str,
    agents: Vec<String>,
    scope: Option<&'static str>,
    /// The version of the copy, when `origin` installed it.
    installed: Option<String>,
    path: Option<String>,
}

fn list(host: &Host, args: ListArgs) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let places = known_places(host, &args.place)?;
    let mut rows = Vec::new();
    for skill in embedded::all() {
        let mut found = false;
        for place in &places {
            let target = place.dir.join(skill.name);
            let Some(copy) = inspect(&target, &skill)? else {
                continue;
            };
            found = true;
            let (status, installed) = match copy {
                Copy::Current(m) => ("current", Some(m.revision)),
                Copy::Outdated(m) => ("outdated", Some(m.revision)),
                Copy::Modified(m) => ("modified", Some(m.revision)),
                Copy::Unmanaged => ("unmanaged", None),
            };
            rows.push(Row {
                name: skill.name,
                version: short(REVISION).to_owned(),
                status,
                agents: place.agents.clone(),
                scope: place.scope.map(Scope::as_str),
                installed: installed.map(|r| short(&r).to_owned()),
                path: Some(target.display().to_string()),
            });
        }
        if !found {
            rows.push(Row {
                name: skill.name,
                version: short(REVISION).to_owned(),
                status: "not-installed",
                agents: Vec::new(),
                scope: None,
                installed: None,
                path: None,
            });
        }
    }
    Ok(Box::new(Rows(rows)))
}

/// The outcome for each skill and place: a table, or a list in JSON.
struct Outcomes(Vec<Outcome>);

impl SerializableOutput for Outcomes {
    fn to_json(&self) -> Result<Value, SerializationError> {
        serde_json::to_value(&self.0).map_err(|e| SerializationError::Other(e.to_string()))
    }

    fn shape(&self) -> Shape {
        listing(&[
            "name", "result", "version", "agents", "scope", "path", "reason",
        ])
    }
}

/// `origin skill list`'s rows, under `skills`.
struct Rows(Vec<Row>);

impl SerializableOutput for Rows {
    fn to_json(&self) -> Result<Value, SerializationError> {
        serde_json::to_value(&self.0).map_err(|e| SerializationError::Other(e.to_string()))
    }

    fn shape(&self) -> Shape {
        listing(&[
            "name",
            "version",
            "status",
            "installed",
            "agents",
            "scope",
            "path",
        ])
    }
}

fn listing(columns: &'static [&'static str]) -> Shape {
    Shape::List(Listing {
        field: "",
        item: None,
        columns,
        parents: Vec::new(),
        create: None,
    })
}
