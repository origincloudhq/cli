//! A resource as its fields, one per line, under their JSON names, so that
//! what is read here is what `jq` finds in `--output json`.

use serde_json::{Map, Value};

use super::text;

/// Between a field's name, padded to the longest of its level, and its
/// value.
const GAP: &str = "  ";

/// How much a nested message or a list's items are indented.
const STEP: usize = 2;

/// What a `null` field shows.
const NONE: &str = "<none>";

/// `value` as lines: an object's fields with what tells it apart and its
/// state first, then its other scalars by name, then its messages and
/// lists of them, each indented under its name; a list's items each
/// marked `- `. A value with line breaks, such as a private key, is
/// printed under its name as it is, unindented, so that it can be copied
/// whole. A field that is `[]` or `{}` isn't shown, as the API's JSON
/// leaves those out; one that is `null`, which only a hand-written
/// command's JSON holds, says so ([`NONE`]): the profile's settings that
/// aren't set.
pub(super) fn detail(value: &Value) -> String {
    let mut out = String::new();
    match value {
        Value::Object(fields) => push_object(&mut out, fields, 0),
        Value::Array(items) => push_items(&mut out, items, 0),
        other => {
            if let Some(value) = text::scalar(other) {
                out.push_str(&text::lines(&value));
                out.push('\n');
            }
        }
    }
    out
}

fn push_object(out: &mut String, fields: &Map<String, Value>, indent: usize) {
    let mut fields: Vec<(&String, &Value)> = fields
        .iter()
        .filter(|(key, value)| !key.starts_with("__") && !is_empty(value))
        .collect();
    // Stable: fields of the same rank keep the JSON's order, by name.
    fields.sort_by_key(|(key, value)| (single(value).is_none(), text::rank(key)));

    let pad = fields
        .iter()
        .filter(|(_, value)| single(value).is_some_and(|v| !v.contains('\n')))
        .map(|(key, _)| text::width(&text::one_line(key)))
        .max()
        .unwrap_or(0);

    for (key, value) in fields {
        let key = text::one_line(key);
        let margin = " ".repeat(indent);
        match single(value) {
            Some(value) if !value.contains('\n') => {
                let padding = " ".repeat(pad - text::width(&key));
                out.push_str(&format!(
                    "{margin}{key}{padding}{GAP}{}\n",
                    text::one_line(&value)
                ));
            }
            Some(value) => {
                out.push_str(&format!("{margin}{key}\n{}", text::lines(&value)));
                if !value.ends_with('\n') {
                    out.push('\n');
                }
            }
            None => {
                out.push_str(&format!("{margin}{key}\n"));
                match value {
                    Value::Object(fields) => push_object(out, fields, indent + STEP),
                    Value::Array(items) => push_items(out, items, indent + STEP),
                    _ => {}
                }
            }
        }
    }
}

fn push_items(out: &mut String, items: &[Value], indent: usize) {
    let margin = " ".repeat(indent);
    for item in items {
        match item {
            Value::Object(fields) => {
                // The item's fields one level in, the first line's indent
                // replaced by the item's mark.
                let mut block = String::new();
                push_object(&mut block, fields, indent + STEP);
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

/// A field's value when it goes on its name's line, or under it as it is.
fn single(value: &Value) -> Option<String> {
    match value {
        Value::Null => Some(NONE.to_owned()),
        value => text::scalar(value),
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

    #[test]
    fn a_resource_is_its_name_and_state_then_its_fields_then_its_messages() {
        let printed = detail(&json!({
            "createTime": "2026-10-31T09:12:00Z",
            "instanceType": "s1.small",
            "name": "projects/acme/zones/z1/instances/web-1",
            "networkInterfaces": [
                {"name": "nic-0", "privateIp": "10.0.1.4"},
                {"name": "nic-1", "privateIp": "10.0.2.4"},
            ],
            "state": "RUNNING",
            "tags": {"env": "prod", "team": "web"},
            "zones": ["z1", "z2"],
        }));
        assert_eq!(
            printed,
            "\
name          projects/acme/zones/z1/instances/web-1
state         RUNNING
instanceType  s1.small
zones         z1, z2
createTime    2026-10-31T09:12:00Z
networkInterfaces
  - name       nic-0
    privateIp  10.0.1.4
  - name       nic-1
    privateIp  10.0.2.4
tags
  env   prod
  team  web
"
        );
    }

    #[test]
    fn what_the_json_would_leave_out_isn_t_shown_and_null_says_so() {
        let printed = detail(&json!({"name": "a", "labels": {}, "zones": [], "region": null}));
        assert_eq!(printed, "name    a\nregion  <none>\n");
    }

    #[test]
    fn a_value_with_line_breaks_is_printed_whole_and_unindented() {
        let key =
            "-----BEGIN OPENSSH PRIVATE KEY-----\nb3Blbn\n-----END OPENSSH PRIVATE KEY-----\n";
        let printed = detail(&json!({"keyPair": {"name": "k", "privateKey": key}}));
        assert_eq!(printed, format!("keyPair\n  name  k\n  privateKey\n{key}"));
    }

    #[test]
    fn an_operation_shows_its_outcome() {
        let printed = detail(&json!({
            "name": "operations/op-1",
            "done": true,
            "error": {"code": 9, "message": "the volume is attached"},
        }));
        assert_eq!(
            printed,
            "\
name  operations/op-1
done  true
error
  code     9
  message  the volume is attached
"
        );
    }

    #[test]
    fn a_value_can_t_carry_terminal_controls() {
        let printed = detail(&json!({"a\u{1b}": "b\u{1b}[31m"}));
        assert_eq!(printed, "a\\u{1b}  b\\u{1b}[31m\n");
    }
}
