//! A resource as its fields, one per line, under their JSON names, so that
//! what is read here is what `jq` finds in `--output json`.

use chrono::{DateTime, Utc};
use prost_reflect::MessageDescriptor;
use serde_json::{Map, Value};

use super::schema::{self, Field, Kind};
use super::text;
use super::value::{self, Place, Shown};

/// Between a field's name, padded to the longest of its level, and its
/// value.
const GAP: &str = "  ";

/// How much a nested message or a list's items are indented.
const STEP: usize = 2;

/// What a `null` field shows.
const NONE: &str = "<none>";

/// `value`, a `message` when its descriptor is known, as lines: an
/// object's fields with what tells it apart and its state first, then its
/// other scalars in the order the API defines them (by name without
/// descriptors), then its messages and lists of them, each indented under
/// its name; a list's items each marked `- `. A timestamp says how long
/// ago it was, and a state is in its colour. A value with line breaks,
/// such as a private key, is printed under its name as it is, unindented,
/// so that it can be copied whole. A field that is `[]` or `{}` isn't
/// shown, as the API's JSON leaves those out; one that is `null`, which
/// only a hand-written command's JSON holds, says so ([`NONE`]): the
/// profile's settings that aren't set.
pub(super) fn detail(
    value: &Value,
    message: Option<&MessageDescriptor>,
    now: DateTime<Utc>,
) -> String {
    let mut out = String::new();
    let lines = Lines { now };
    match value {
        Value::Object(fields) => lines.object(&mut out, fields, message, 0),
        Value::Array(items) => lines.items(&mut out, items, message, 0),
        other => {
            if let Some(value) = text::scalar(other) {
                out.push_str(&text::lines(&value));
                out.push('\n');
            }
        }
    }
    out
}

struct Lines {
    now: DateTime<Utc>,
}

/// A field of an object, ready to be printed.
struct Entry<'a> {
    key: String,
    value: &'a Value,
    field: Field,
    /// What goes on its name's line; `None` for a block under it.
    line: Option<Shown>,
    /// Where the API defines it, for the order.
    index: usize,
}

impl Lines {
    fn object(
        &self,
        out: &mut String,
        fields: &Map<String, Value>,
        message: Option<&MessageDescriptor>,
        indent: usize,
    ) {
        // An `Any` says which message it holds.
        let any = schema::any_message(fields);
        let message = any.as_ref().or(message);

        let mut entries: Vec<Entry<'_>> = fields
            .iter()
            .filter(|(key, value)| !key.starts_with("__") && !is_empty(value))
            .map(|(key, value)| {
                let described = message.and_then(|m| schema::field(m, key));
                let field = described
                    .as_ref()
                    .map(Field::of)
                    .unwrap_or_else(|| Field::told_from(value));
                let line = match value {
                    Value::Null => Some(Shown {
                        text: NONE.to_owned(),
                        style: None,
                    }),
                    value => value::show(value, &field, Place::Detail, self.now),
                };
                Entry {
                    key: text::one_line(key),
                    value,
                    field,
                    line,
                    index: described
                        .and_then(|d| message?.fields().position(|f| f == d))
                        .unwrap_or(usize::MAX),
                }
            })
            .collect();
        // Stable: without descriptors, the JSON's order, by name.
        entries.sort_by_key(|e| {
            let block = e.line.as_ref().is_none_or(|l| l.text.contains('\n'));
            (block, text::rank(&e.key), e.index)
        });

        let pad = entries
            .iter()
            .filter(|e| e.line.as_ref().is_some_and(|l| !l.text.contains('\n')))
            .map(|e| text::width(&e.key))
            .max()
            .unwrap_or(0);
        let margin = " ".repeat(indent);

        for entry in entries {
            let key = &entry.key;
            match &entry.line {
                Some(line) if !line.text.contains('\n') => {
                    let padding = " ".repeat(pad - text::width(key));
                    let shown = text::one_line(&line.text);
                    let shown = match line.style {
                        Some(style) => format!("{style}{shown}{style:#}"),
                        None => shown,
                    };
                    out.push_str(&format!("{margin}{key}{padding}{GAP}{shown}\n"));
                }
                Some(line) => {
                    out.push_str(&format!("{margin}{key}\n{}", text::lines(&line.text)));
                    if !line.text.ends_with('\n') {
                        out.push('\n');
                    }
                }
                None => {
                    out.push_str(&format!("{margin}{key}\n"));
                    let inner = match entry.field.kind {
                        Kind::Map => None,
                        _ => entry.field.message(),
                    };
                    match entry.value {
                        Value::Object(fields) => self.object(out, fields, inner, indent + STEP),
                        Value::Array(items) => self.items(out, items, inner, indent + STEP),
                        _ => {}
                    }
                }
            }
        }
    }

    fn items(
        &self,
        out: &mut String,
        items: &[Value],
        message: Option<&MessageDescriptor>,
        indent: usize,
    ) {
        let margin = " ".repeat(indent);
        for item in items {
            match item {
                Value::Object(fields) => {
                    // The item's fields one level in, the first line's
                    // indent replaced by the item's mark.
                    let mut block = String::new();
                    self.object(&mut block, fields, message, indent + STEP);
                    if block.is_empty() {
                        out.push_str(&format!("{margin}-\n"));
                    } else {
                        block.replace_range(indent..indent + STEP, "- ");
                        out.push_str(&block);
                    }
                }
                other => {
                    let value = text::scalar(other).unwrap_or_else(|| other.to_string());
                    out.push_str(&format!("{margin}- {}\n", text::one_line(&value)));
                }
            }
        }
    }
}

fn is_empty(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.is_empty(),
        Value::Object(fields) => fields.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        "2026-11-03T12:00:00Z".parse().unwrap()
    }

    fn plain(value: &Value, message: Option<&str>) -> String {
        let message = message.map(|m| crate::descriptors::message(m).unwrap());
        anstream::adapter::strip_str(&detail(value, message.as_ref(), now())).to_string()
    }

    #[test]
    fn a_resource_is_its_fields_in_the_definitions_order() {
        let printed = plain(
            &json!({
                "createTime": "2026-10-31T09:12:00Z",
                "instanceType": "instanceTypes/s1.small",
                "interfaceAddresses": [
                    {"networkInterface": "nic-0", "ipv4Address": "10.0.1.4"},
                ],
                "name": "projects/acme/zones/z1/instances/web-1",
                "networkInterfaces": ["nic-0", "nic-1"],
                "state": "RUNNING",
                "tags": {"env": "prod", "team": "web"},
            }),
            Some("origincloud.compute.v1beta1.Instance"),
        );
        assert_eq!(
            printed,
            "\
name               projects/acme/zones/z1/instances/web-1
state              RUNNING
instanceType       instanceTypes/s1.small
networkInterfaces  nic-0, nic-1
createTime         2026-10-31T09:12:00Z (3d ago)
tags
  env   prod
  team  web
interfaceAddresses
  - networkInterface  nic-0
    ipv4Address       10.0.1.4
"
        );
    }

    #[test]
    fn a_state_is_in_its_colour() {
        let message = crate::descriptors::message("origincloud.pvs.v1beta1.Volume").unwrap();
        let printed = detail(&json!({"state": "ERROR"}), Some(&message), now());
        assert_ne!(printed, anstream::adapter::strip_str(&printed).to_string());
    }

    #[test]
    fn without_descriptors_the_fields_are_by_name() {
        let printed = plain(
            &json!({
                "name": "a",
                "networkInterfaces": [
                    {"name": "nic-0", "privateIp": "10.0.1.4"},
                    {"name": "nic-1", "privateIp": "10.0.2.4"},
                ],
                "state": "RUNNING",
                "zones": ["z1", "z2"],
            }),
            None,
        );
        assert_eq!(
            printed,
            "\
name   a
state  RUNNING
zones  z1, z2
networkInterfaces
  - name       nic-0
    privateIp  10.0.1.4
  - name       nic-1
    privateIp  10.0.2.4
"
        );
    }

    #[test]
    fn what_the_json_would_leave_out_isn_t_shown_and_null_says_so() {
        let printed = plain(
            &json!({"name": "a", "labels": {}, "zones": [], "region": null}),
            None,
        );
        assert_eq!(printed, "name    a\nregion  <none>\n");
    }

    #[test]
    fn a_value_with_line_breaks_is_printed_whole_and_unindented() {
        let key =
            "-----BEGIN OPENSSH PRIVATE KEY-----\nb3Blbn\n-----END OPENSSH PRIVATE KEY-----\n";
        let printed = plain(&json!({"keyPair": {"name": "k", "privateKey": key}}), None);
        assert_eq!(printed, format!("keyPair\n  name  k\n  privateKey\n{key}"));
    }

    #[test]
    fn an_operation_shows_its_outcome_and_the_message_it_holds() {
        let printed = plain(
            &json!({
                "name": "projects/p/zones/z/operations/op-1",
                "done": true,
                "metadata": {
                    "@type": "type.googleapis.com/origincloud.longrunning.v1beta1.OperationMetadata",
                    "verb": "create",
                    "createTime": "2026-11-03T11:00:00Z",
                },
                "error": {"code": 9, "message": "the volume is attached"},
            }),
            Some("google.longrunning.Operation"),
        );
        assert_eq!(
            printed,
            "\
name  projects/p/zones/z/operations/op-1
done  true
metadata
  verb        create
  createTime  2026-11-03T11:00:00Z (60m ago)
  @type       type.googleapis.com/origincloud.longrunning.v1beta1.OperationMetadata
error
  code     9
  message  the volume is attached
"
        );
    }

    #[test]
    fn a_value_can_t_carry_terminal_controls() {
        let printed = plain(&json!({"a\u{1b}": "b\u{1b}[31m"}), None);
        assert_eq!(printed, "a\\u{1b}  b\\u{1b}[31m\n");
    }
}
