//! The choices the API definitions don't express, from `commands.toml`
//! beside the generator: what a command group's help says, methods left
//! without a command, fields left out of a command, changes that can't be
//! undone, secrets a response shows once, reference fields taking the
//! names of more than one resource type, the columns a list's table shows,
//! and the hand-written groups the generated ones replace.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// `commands.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// The hand-written groups the CLI still has, such as `iam`. A
    /// service named like one, or replacing one, fails generation until it
    /// is retired in the same change.
    #[serde(default)]
    pub hand_written: Vec<String>,
    /// The services replacing a hand-written group under another name,
    /// written before the API has them.
    #[serde(default)]
    pub replacement: Vec<Replacement>,
    /// The hand-written commands the CLI no longer has, with nothing of the
    /// same name in their place, with the reason.
    #[serde(default)]
    #[allow(dead_code)]
    pub retired: Vec<Retired>,
    /// The help of command groups.
    #[serde(default)]
    pub service: Vec<Group>,
    /// Methods left without a command.
    #[serde(default)]
    pub no_command: Vec<NoCommand>,
    /// Fields left out of a method's command.
    #[serde(default)]
    pub exclude: Vec<Exclusion>,
    /// Fields whose change can't be undone: setting one needs `--yes`.
    #[serde(default)]
    pub one_way: Vec<OneWay>,
    /// Fields of a response holding a secret the API returns in that
    /// response alone.
    #[serde(default)]
    pub shown_once: Vec<ShownOnce>,
    /// String fields of a request a command reads from a file.
    #[serde(default)]
    pub from_file: Vec<FromFile>,
    /// Reference fields that also take the names of another resource type.
    #[serde(default)]
    pub also_names: Vec<AlsoNames>,
    /// The columns of the table a list of a message is printed as.
    #[serde(default)]
    pub view: Vec<View>,
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

/// A service replacing a hand-written group although it isn't named like
/// it, such as `resourcemanager` replacing `admin`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replacement {
    /// The hand-written group, such as `admin`.
    pub group: String,
    /// The service, as its package names it, such as `resourcemanager`.
    pub service: String,
}

/// A hand-written command the CLI no longer has, which no command of the
/// same name replaces.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct Retired {
    /// The hand-written group, such as `admin`.
    pub group: String,
    /// Its command, such as `register-organisation`; none for a group that
    /// is one command, such as `version`.
    #[serde(default)]
    pub command: String,
    /// Why nothing replaces it, and what to use instead.
    pub why: String,
}

/// A method left without a command.
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

/// A field of a method's response holding a secret the API returns in that
/// response alone, such as a generated private key: the command says so on
/// standard error when the response carries one.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShownOnce {
    pub service: String,
    pub method: String,
    /// The field of the response, such as `private_key`.
    pub field: String,
    /// What standard error says: that it can't be had again, and how to
    /// keep it.
    pub notice: String,
}

/// A string field of a request itself a command reads from a file, or from
/// standard input, rather than the command line, where it would be in the
/// shell's history and every process listing: its flag is `--{field}-file`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FromFile {
    pub service: String,
    pub method: String,
    /// The field of the request, such as `token`.
    pub field: String,
    /// Why, for whoever reads the table.
    #[allow(dead_code)]
    pub why: String,
}

/// A reference field that also takes the names of a resource type other
/// than the one its `google.api.resource_reference` declares, which can
/// name one type only: its flag takes a whole name of either, and an id
/// still completes to the declared type's pattern.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlsoNames {
    /// The field, by its full name, wherever it is a flag or a key:
    /// `origincloud.compute.v1beta1.Instance.image`.
    pub field: String,
    /// The other resource type, such as
    /// `compute.origincloudapis.eu/PublicImage`.
    pub resource: String,
    /// Why, for whoever reads the table.
    #[allow(dead_code)]
    pub why: String,
}

/// The columns of the `table` a list of a message is printed as, which
/// would otherwise be every field the message defines that fits in a cell.
/// `wide` shows those after these.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    /// The service, as its packages name it: `compute`.
    pub service: String,
    /// The message, as the package names it: `Instance`.
    pub message: String,
    /// The fields, as paths from the message through messages and lists of
    /// them, such as `interface_addresses.ipv4_address`, in order.
    pub columns: Vec<String>,
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

    /// The hand-written groups `service` collides with or replaces, which
    /// are still there.
    pub fn superseded(&self, service: &str) -> Vec<&str> {
        self.hand_written
            .iter()
            .filter(|h| {
                *h == service
                    || self
                        .replacement
                        .iter()
                        .any(|r| r.service == service && r.group == **h)
            })
            .map(String::as_str)
            .collect()
    }

    pub fn no_command(&self, service: &str, method: &str) -> bool {
        self.no_command
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

    pub fn read_from_file(&self, service: &str, method: &str, field: &str) -> bool {
        self.from_file
            .iter()
            .any(|f| f.service == service && f.method == method && f.field == field)
    }

    pub fn shown_once(&self, service: &str, method: &str) -> Vec<&ShownOnce> {
        self.shown_once
            .iter()
            .filter(|s| s.service == service && s.method == method)
            .collect()
    }

    /// Every `(service, method, field)` the table names, the field a path
    /// from the request; the field is empty for a method left without a
    /// command. The fields of `shown_once`, which are a response's, aren't
    /// among them: generation fails for one the response doesn't have.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str, &str)> {
        let no_command = self
            .no_command
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
        let from_file = self
            .from_file
            .iter()
            .map(|f| (f.service.as_str(), f.method.as_str(), f.field.as_str()));
        no_command.chain(exclude).chain(one_way).chain(from_file)
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

            [[exclude]]
            service = "pvs"
            method = "ImportVolume"
            field = "volume.source_snapshot"
            why = "y"

            [[shown_once]]
            service = "compute"
            method = "CreateKeyPair"
            field = "private_key"
            notice = "z"

            [[from_file]]
            service = "sts"
            method = "AssumeServiceAccount"
            field = "token"
            why = "v"

            [[also_names]]
            field = "origincloud.compute.v1beta1.Instance.image"
            resource = "compute.origincloudapis.eu/PublicImage"
            why = "w"

            [[view]]
            service = "compute"
            message = "Instance"
            columns = ["name", "state"]
            "#,
        )
        .unwrap();
        assert_eq!(policy.shown_once("compute", "CreateKeyPair")[0].notice, "z");
        assert!(policy.shown_once("compute", "GetKeyPair").is_empty());
        assert_eq!(policy.view[0].columns, ["name", "state"]);
        assert_eq!(policy.group("pvs").unwrap().about, "Volumes");
        assert!(policy.no_command("longrunning", "WaitOperation"));
        assert!(!policy.no_command("pvs", "WaitOperation"));
        assert!(policy.excluded("pvs", "ImportVolume", "volume.source_snapshot"));
        assert!(!policy.excluded("other", "ImportVolume", "volume.source_snapshot"));
        assert!(policy.read_from_file("sts", "AssumeServiceAccount", "token"));
        assert!(!policy.read_from_file("sts", "AssumeServiceAccount", "lifetime"));
        assert_eq!(policy.entries().count(), 3);
        assert_eq!(
            policy.also_names[0].resource,
            "compute.origincloudapis.eu/PublicImage"
        );
        // Every entry says why.
        assert!(
            toml::from_str::<Policy>("[[no_command]]\nservice = \"pvs\"\nmethod = \"X\"\n")
                .is_err()
        );
    }
}
