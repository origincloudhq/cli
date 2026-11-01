//! The choices the API definitions don't express, from `commands.toml`
//! beside the generator: fields left out of a command, and changes that
//! can't be undone.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// `commands.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Fields left out of a method's command.
    #[serde(default)]
    pub exclude: Vec<Exclusion>,
    /// Fields whose change can't be undone: setting one needs `--yes`.
    #[serde(default)]
    pub one_way: Vec<OneWay>,
}

/// A field left out of a method's command.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exclusion {
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

    pub fn excluded(&self, method: &str, field: &str) -> bool {
        self.exclude
            .iter()
            .any(|e| e.method == method && e.field == field)
    }

    pub fn one_way(&self, method: &str, field: &str) -> Option<&OneWay> {
        self.one_way
            .iter()
            .find(|o| o.method == method && o.field == field)
    }

    /// Every `(method, field)` the table names.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.exclude
            .iter()
            .map(|e| (e.method.as_str(), e.field.as_str()))
            .chain(
                self.one_way
                    .iter()
                    .map(|o| (o.method.as_str(), o.field.as_str())),
            )
    }
}
