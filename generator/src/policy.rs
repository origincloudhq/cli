//! The choices the API definitions don't express, from `commands.toml`
//! beside the generator: what a command group's help says, methods left
//! without a command or without an MCP tool, fields left out of a command
//! and its tool, and changes that can't be undone.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// `commands.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// The help of command groups.
    #[serde(default)]
    pub service: Vec<Group>,
    /// Methods left without a command.
    #[serde(default)]
    pub no_command: Vec<NoCommand>,
    /// Methods left without an MCP tool.
    #[serde(default)]
    pub no_tool: Vec<NoCommand>,
    /// Fields left out of a method's command.
    #[serde(default)]
    pub exclude: Vec<Exclusion>,
    /// Fields whose change can't be undone: setting one needs `--yes`.
    #[serde(default)]
    pub one_way: Vec<OneWay>,
}

/// The help of a service's command group, when its definitions don't say
/// enough: a package with several services has no one comment to take it
/// from.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    /// The service, as its packages name it: `pvs`.
    pub name: String,
    /// The group's line in `origin --help`.
    pub about: String,
    /// What `origin {name} --help` adds below it.
    #[serde(default)]
    pub details: String,
}

/// A method left without a command, or without an MCP tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoCommand {
    /// The service, as its packages name it: `pvs`, or `longrunning` for
    /// the operation methods every group has.
    pub service: String,
    /// The method, such as `GetVolume`.
    pub method: String,
    /// Why, for whoever reads the table.
    #[allow(dead_code)]
    pub why: String,
}

/// A field left out of a method's command.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exclusion {
    /// The service, as its packages name it: `pvs`.
    pub service: String,
    /// The method, such as `ImportVolume`.
    pub method: String,
    /// The field, as a path from the request, such as
    /// `volume.source_snapshot`.
    pub field: String,
    /// Why, for whoever reads the table.
    #[allow(dead_code)]
    pub why: String,
}

/// A field whose change can't be undone.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OneWay {
    pub service: String,
    pub method: String,
    pub field: String,
    /// The refusal's verb: `refusing to {verb} volume …`.
    pub verb: String,
    /// What can't be undone, for someone deciding whether to go on.
    pub consequence: String,
}

impl Policy {
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("reading {}", path.display()))
    }

    pub fn group(&self, service: &str) -> Option<&Group> {
        self.service.iter().find(|g| g.name == service)
    }

    pub fn no_command(&self, service: &str, method: &str) -> bool {
        self.no_command
            .iter()
            .any(|n| n.service == service && n.method == method)
    }

    pub fn no_tool(&self, service: &str, method: &str) -> bool {
        self.no_tool
            .iter()
            .any(|n| n.service == service && n.method == method)
    }

    pub fn excluded(&self, service: &str, method: &str, field: &str) -> bool {
        self.exclude
            .iter()
            .any(|e| e.service == service && e.method == method && e.field == field)
    }

    pub fn one_way(&self, service: &str, method: &str, field: &str) -> Option<&OneWay> {
        self.one_way
            .iter()
            .find(|o| o.service == service && o.method == method && o.field == field)
    }

    /// Every `(service, method, field)` the table names; the field is empty
    /// for a method left without a command or a tool.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str, &str)> {
        let no_command = self
            .no_command
            .iter()
            .map(|n| (n.service.as_str(), n.method.as_str(), ""));
        let no_tool = self
            .no_tool
            .iter()
            .map(|n| (n.service.as_str(), n.method.as_str(), ""));
        let exclude = self
            .exclude
            .iter()
            .map(|e| (e.service.as_str(), e.method.as_str(), e.field.as_str()));
        let one_way = self
            .one_way
            .iter()
            .map(|o| (o.service.as_str(), o.method.as_str(), o.field.as_str()));
        no_command.chain(no_tool).chain(exclude).chain(one_way)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_parses_and_answers_by_service() {
        let policy: Policy = toml::from_str(
            r#"
            [[service]]
            name = "pvs"
            about = "Volumes"

            [[no_command]]
            service = "longrunning"
            method = "WaitOperation"
            why = "x"

            [[no_tool]]
            service = "pvs"
            method = "ImportVolume"
            why = "z"

            [[exclude]]
            service = "pvs"
            method = "ImportVolume"
            field = "volume.source_snapshot"
            why = "y"
            "#,
        )
        .unwrap();
        assert_eq!(policy.group("pvs").unwrap().about, "Volumes");
        assert!(policy.no_command("longrunning", "WaitOperation"));
        assert!(!policy.no_command("pvs", "WaitOperation"));
        assert!(policy.no_tool("pvs", "ImportVolume"));
        assert!(!policy.no_tool("longrunning", "WaitOperation"));
        assert!(policy.excluded("pvs", "ImportVolume", "volume.source_snapshot"));
        assert!(!policy.excluded("other", "ImportVolume", "volume.source_snapshot"));
        assert_eq!(policy.entries().count(), 3);
        // Every entry says why.
        assert!(
            toml::from_str::<Policy>("[[no_command]]\nservice = \"pvs\"\nmethod = \"X\"\n")
                .is_err()
        );
    }
}
