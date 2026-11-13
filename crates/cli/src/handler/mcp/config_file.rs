//! Reading and changing an agent's MCP configuration file, as text: the
//! entry of one server, by its name, in the table of servers its
//! [`Format`] names.
//!
//! An edit changes that entry alone and keeps the rest of the file as it
//! was, byte for byte: the other servers and settings, their order, the
//! indentation, and the comments where the syntax has them (JSON with
//! comments, TOML). Within the entry, it sets `command`, `args` and, where
//! the format has them, `type` and `tools`, and keeps any other key someone added, such
//! as `env` or a timeout. A file that doesn't parse is an error, never
//! overwritten.

use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use jsonc_parser::ParseOptions;
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use super::agents::{Format, Syntax};

/// The server entry `origin mcp install` writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

/// What a file's entry of a name holds, of the keys an install sets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Found {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub kind: Option<String>,
    pub tools: Option<Vec<String>>,
}

impl Found {
    /// Whether it starts `origin mcp serve`: a program called `origin`
    /// with the arguments `mcp serve`. An entry that doesn't is someone
    /// else's, which neither install nor uninstall touches.
    pub fn is_origin(&self) -> bool {
        let program = self.command.as_deref().map(|c| {
            std::path::Path::new(c)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned()
        });
        program.as_deref() == Some("origin")
            && self
                .args
                .as_deref()
                .unwrap_or_default()
                .windows(2)
                .any(|w| w[0] == "mcp" && w[1] == "serve")
    }

    /// Whether it is `entry` as `format` writes it.
    pub fn is(&self, entry: &Entry, format: Format) -> bool {
        self.command.as_deref() == Some(entry.command.as_str())
            && self.args.as_deref() == Some(entry.args.as_slice())
            && (!format.stdio_type || self.kind.as_deref() == Some("stdio"))
            && (!format.all_tools || self.tools.as_deref() == Some(&["*".to_owned()][..]))
    }
}

/// What an edit did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Change {
    Added,
    Updated,
    Unchanged,
    Removed,
    Absent,
}

/// A file's content that doesn't parse, or whose servers aren't a table.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct Invalid(String);

/// The entry `name` in `text`, if it has one.
pub(crate) fn find(format: Format, text: &str, name: &str) -> Result<Option<Found>, Invalid> {
    match format.syntax {
        Syntax::Json | Syntax::Jsonc => json::find(format, text, name),
        Syntax::Toml => toml::find(format, text, name),
    }
}

/// `text` (`None` for a file that doesn't exist yet) with `entry` added, or
/// updated to it.
pub(crate) fn install(
    format: Format,
    text: Option<&str>,
    entry: &Entry,
) -> Result<(String, Change), Invalid> {
    match format.syntax {
        Syntax::Json | Syntax::Jsonc => json::install(format, text, entry),
        Syntax::Toml => toml::install(format, text, entry),
    }
}

/// `text` without the entry `name`.
pub(crate) fn uninstall(
    format: Format,
    text: &str,
    name: &str,
) -> Result<(String, Change), Invalid> {
    match format.syntax {
        Syntax::Json | Syntax::Jsonc => json::uninstall(format, text, name),
        Syntax::Toml => toml::uninstall(format, text, name),
    }
}

/// `entry` alone, as `format` writes it: what `--print` prints, for a
/// file of that format.
pub(crate) fn snippet(format: Format, entry: &Entry) -> String {
    let (text, _) = install(format, None, entry).expect("an empty file takes an entry");
    text
}

mod json {
    use super::*;

    /// JSON with comments, as VS Code and the agents built on it read it:
    /// comments and trailing commas, and nothing else beyond JSON.
    fn options() -> ParseOptions {
        ParseOptions {
            allow_comments: true,
            allow_trailing_commas: true,
            allow_loose_object_property_names: false,
            allow_missing_commas: false,
            allow_single_quoted_strings: false,
            allow_hexadecimal_numbers: false,
            allow_unary_plus_numbers: false,
            allow_bare_decimal_point_numbers: false,
            allow_non_finite_numbers: false,
            allow_extended_string_escapes: false,
        }
    }

    fn parse(format: Format, text: &str) -> Result<(CstRootNode, CstObject), Invalid> {
        // An empty file is an empty object, as the agents read it.
        let text = if text.trim().is_empty() { "{}\n" } else { text };
        // The syntax tree's parser takes comments whatever its options say,
        // so a file whose agent reads strict JSON is checked as that first.
        if format.syntax == Syntax::Json {
            serde_json::from_str::<serde_json::Value>(text)
                .map_err(|e| Invalid(format!("isn't valid JSON: {e}")))?;
        }
        let root = CstRootNode::parse(text, &options())
            .map_err(|e| Invalid(format!("isn't valid {}: {e}", format.syntax.describe())))?;
        let object = root
            .object_value()
            .ok_or_else(|| Invalid("doesn't hold a JSON object".to_owned()))?;
        Ok((root, object))
    }

    /// The table of servers, which `create` adds when there is none.
    fn servers(
        format: Format,
        root: &CstObject,
        create: bool,
    ) -> Result<Option<CstObject>, Invalid> {
        let not_an_object = || Invalid(format!("its `{}` isn't an object", format.servers));
        match root.get(format.servers) {
            Some(prop) => prop.object_value().map(Some).ok_or_else(not_an_object),
            None if create => Ok(Some(root.object_value_or_set(format.servers))),
            None => Ok(None),
        }
    }

    fn entry_object(
        format: Format,
        servers: &CstObject,
        name: &str,
    ) -> Result<Option<CstObject>, Invalid> {
        match servers.get(name) {
            Some(prop) => prop
                .object_value()
                .map(Some)
                .ok_or_else(|| Invalid(format!("its `{}.{name}` isn't an object", format.servers))),
            None => Ok(None),
        }
    }

    pub fn find(format: Format, text: &str, name: &str) -> Result<Option<Found>, Invalid> {
        let (_, root) = parse(format, text)?;
        let Some(servers) = servers(format, &root, false)? else {
            return Ok(None);
        };
        let Some(entry) = entry_object(format, &servers, name)? else {
            return Ok(None);
        };
        let value = entry.to_serde_value().unwrap_or_default();
        let string = |key: &str| value.get(key).and_then(|v| v.as_str()).map(str::to_owned);
        let strings = |key: &str| {
            value.get(key).and_then(|v| v.as_array()).map(|items| {
                items
                    .iter()
                    .map(|a| a.as_str().unwrap_or_default().to_owned())
                    .collect()
            })
        };
        Ok(Some(Found {
            command: string("command"),
            args: strings("args"),
            kind: string("type"),
            tools: strings("tools"),
        }))
    }

    fn fields(format: Format, entry: &Entry) -> Vec<(String, CstInputValue)> {
        let mut fields = Vec::new();
        if format.stdio_type {
            fields.push(("type".to_owned(), CstInputValue::from("stdio")));
        }
        fields.push((
            "command".to_owned(),
            CstInputValue::from(entry.command.as_str()),
        ));
        fields.push((
            "args".to_owned(),
            CstInputValue::Array(entry.args.iter().map(|a| a.as_str().into()).collect()),
        ));
        if format.all_tools {
            fields.push(("tools".to_owned(), CstInputValue::Array(vec!["*".into()])));
        }
        fields
    }

    pub fn install(
        format: Format,
        text: Option<&str>,
        entry: &Entry,
    ) -> Result<(String, Change), Invalid> {
        let (root_node, root) = parse(format, text.unwrap_or_default())?;
        if let Some(found) = text
            .map(|t| find(format, t, &entry.name))
            .transpose()?
            .flatten()
        {
            if found.is(entry, format) {
                return Ok((text.unwrap_or_default().to_owned(), Change::Unchanged));
            }
        }
        let servers = servers(format, &root, true)?.expect("created");
        let change = match entry_object(format, &servers, &entry.name)? {
            Some(existing) => {
                for (key, value) in fields(format, entry) {
                    match existing.get(&key) {
                        Some(prop) => prop.set_value(value),
                        None => {
                            existing.append(&key, value);
                        }
                    }
                }
                Change::Updated
            }
            None => {
                servers.append(&entry.name, CstInputValue::Object(fields(format, entry)));
                Change::Added
            }
        };
        let mut out = root_node.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        Ok((out, change))
    }

    pub fn uninstall(format: Format, text: &str, name: &str) -> Result<(String, Change), Invalid> {
        let (root_node, root) = parse(format, text)?;
        let Some(servers) = servers(format, &root, false)? else {
            return Ok((text.to_owned(), Change::Absent));
        };
        match servers.get(name) {
            Some(prop) => {
                prop.remove();
                Ok((root_node.to_string(), Change::Removed))
            }
            None => Ok((text.to_owned(), Change::Absent)),
        }
    }
}

mod toml {
    use super::*;

    fn parse(text: &str) -> Result<DocumentMut, Invalid> {
        text.parse::<DocumentMut>()
            .map_err(|e| Invalid(format!("isn't valid TOML: {}", e.message())))
    }

    fn servers(
        format: Format,
        doc: &mut DocumentMut,
        create: bool,
    ) -> Result<Option<&mut Table>, Invalid> {
        if !doc.contains_key(format.servers) {
            if !create {
                return Ok(None);
            }
            let mut table = Table::new();
            // `[mcp_servers.name]` alone, without an empty `[mcp_servers]`.
            table.set_implicit(true);
            doc.insert(format.servers, Item::Table(table));
        }
        doc.get_mut(format.servers)
            .and_then(Item::as_table_mut)
            .map(Some)
            .ok_or_else(|| Invalid(format!("its `{}` isn't a table", format.servers)))
    }

    pub fn find(format: Format, text: &str, name: &str) -> Result<Option<Found>, Invalid> {
        let mut doc = parse(text)?;
        let Some(servers) = servers(format, &mut doc, false)? else {
            return Ok(None);
        };
        let Some(item) = servers.get(name) else {
            return Ok(None);
        };
        let entry = item
            .as_table_like()
            .ok_or_else(|| Invalid(format!("its `{}.{name}` isn't a table", format.servers)))?;
        let string = |key: &str| entry.get(key).and_then(Item::as_str).map(str::to_owned);
        Ok(Some(Found {
            command: string("command"),
            args: entry.get("args").and_then(Item::as_array).map(|args| {
                args.iter()
                    .map(|a| a.as_str().unwrap_or_default().to_owned())
                    .collect()
            }),
            kind: string("type"),
            tools: None,
        }))
    }

    pub fn install(
        format: Format,
        text: Option<&str>,
        entry: &Entry,
    ) -> Result<(String, Change), Invalid> {
        let text = text.unwrap_or_default();
        if let Some(found) = find(format, text, &entry.name)? {
            if found.is(entry, format) {
                return Ok((text.to_owned(), Change::Unchanged));
            }
        }
        let mut doc = parse(text)?;
        let servers = servers(format, &mut doc, true)?.expect("created");
        let args = Value::Array(entry.args.iter().map(String::as_str).collect::<Array>());
        let change = match servers.get_mut(&entry.name) {
            Some(existing) => {
                let existing = existing.as_table_like_mut().expect("find checked it");
                existing.insert("command", toml_edit::value(entry.command.as_str()));
                existing.insert("args", Item::Value(args));
                Change::Updated
            }
            None => {
                let mut table = Table::new();
                table.insert("command", toml_edit::value(entry.command.as_str()));
                table.insert("args", Item::Value(args));
                servers.insert(&entry.name, Item::Table(table));
                Change::Added
            }
        };
        Ok((doc.to_string(), change))
    }

    pub fn uninstall(format: Format, text: &str, name: &str) -> Result<(String, Change), Invalid> {
        let mut doc = parse(text)?;
        let Some(servers) = servers(format, &mut doc, false)? else {
            return Ok((text.to_owned(), Change::Absent));
        };
        match servers.remove(name) {
            Some(_) => Ok((doc.to_string(), Change::Removed)),
            None => Ok((text.to_owned(), Change::Absent)),
        }
    }
}

#[cfg(test)]
mod tests;
