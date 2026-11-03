//! The text the human formats are made of: a JSON field's name as words,
//! a value as one line, and a line's width on a terminal.

use serde_json::Value;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// The words of a JSON field's name, lowercase: `instance type` for
/// `instanceType`, `ip address` for `ipAddress`, `vpc id` for `vpc_id`.
pub(super) fn words(key: &str) -> String {
    let mut words = String::new();
    let mut previous: Option<char> = None;
    for c in key.chars() {
        if c == '_' || c == '-' {
            if !words.is_empty() && !words.ends_with(' ') {
                words.push(' ');
            }
        } else if c.is_uppercase() {
            if previous.is_some_and(|p| p.is_lowercase() || p.is_ascii_digit()) {
                words.push(' ');
            }
            words.extend(c.to_lowercase());
        } else {
            words.push(c);
        }
        previous = Some(c);
    }
    words.trim_end().to_owned()
}

/// A table column's header for the field `key`: `INSTANCE TYPE` for
/// `instanceType`.
pub(super) fn header(key: &str) -> String {
    words(key).to_uppercase()
}

/// Where a field goes among an item's: what tells the items apart and
/// what state each is in first, when it was made last, the others between.
pub(super) fn rank(key: &str) -> u8 {
    match key {
        "name" => 0,
        "displayName" => 1,
        "state" => 2,
        "status" => 3,
        "createTime" => 5,
        // What message an `Any` holds, after what it holds.
        "@type" => 6,
        _ => 4,
    }
}

/// The last segment of a resource name, which is what tells its resource
/// apart from the others of a list: `web-1` for
/// `projects/acme/zones/eu-central1-az1/instances/web-1`.
pub(super) fn short_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// Where a list looked, from the parents it listed, in words: `in project
/// acme, zone z1` for `projects/acme/zones/z1`; `in any zone of project
/// acme` for several zones of one project. `None` for a list without a
/// parent, or a parent that isn't a resource name.
pub(super) fn scope(parents: &[String]) -> Option<String> {
    let named: Vec<Vec<(&str, &str)>> = parents
        .iter()
        .map(|parent| pairs(parent))
        .collect::<Option<_>>()?;
    let first = named.first()?;
    let mut same = Vec::new();
    let mut any = Vec::new();
    for (i, (collection, id)) in first.iter().enumerate() {
        let collection = singular(collection);
        if named.iter().all(|n| n.get(i) == first.get(i)) {
            same.push(format!("{collection} {id}"));
        } else {
            any.push(collection);
        }
    }
    Some(match (any.is_empty(), same.is_empty()) {
        (true, _) => format!("in {}", same.join(", ")),
        (false, true) => format!("in any {}", any.join(" and ")),
        (false, false) => format!("in any {} of {}", any.join(" and "), same.join(", ")),
    })
}

/// A resource name's collections and ids: `[("projects", "acme"),
/// ("zones", "z1")]`.
fn pairs(name: &str) -> Option<Vec<(&str, &str)>> {
    let segments: Vec<&str> = name.split('/').collect();
    if name.is_empty() || !segments.len().is_multiple_of(2) || segments.iter().any(|s| s.is_empty())
    {
        return None;
    }
    Some(segments.chunks(2).map(|pair| (pair[0], pair[1])).collect())
}

/// A collection's resource, in words: `zone` for `zones`, `dns policy`
/// for `dnsPolicies`.
fn singular(collection: &str) -> String {
    let one = if let Some(stem) = collection.strip_suffix("ies") {
        format!("{stem}y")
    } else {
        collection
            .strip_suffix('s')
            .unwrap_or(collection)
            .to_owned()
    };
    words(&one)
}

/// `value` as text: a string as it is, a number or a boolean as JSON
/// writes it, a list of them joined with `, `. `None` for what isn't one
/// of those: an object, a list holding one, `null`.
pub(super) fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Array(_) => None,
                item => scalar(item),
            })
            .collect::<Option<Vec<_>>>()
            .map(|items| items.join(", ")),
        Value::Object(_) | Value::Null => None,
    }
}

/// `text` safe to print on a terminal, on one line: a line break or a tab
/// becomes a space, and any other control character is written as its
/// escape (`\u{1b}`), so that a value from the API can't move the cursor,
/// change colours or retitle the window.
pub(super) fn one_line(text: &str) -> String {
    let mut line = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' | '\r' | '\t' => line.push(' '),
            c if c.is_control() => line.extend(c.escape_unicode()),
            c => line.push(c),
        }
    }
    line
}

/// `text` safe to print on a terminal, its line breaks kept: as
/// [`one_line`] makes it, but for `\n`.
pub(super) fn lines(text: &str) -> String {
    text.split('\n')
        .map(one_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// How many columns `text` takes on a terminal.
pub(super) fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// `text` cut to `max` columns, ending in `…` when it was cut.
pub(super) fn truncate(text: &str, max: usize) -> String {
    if width(text) <= max {
        return text.to_owned();
    }
    let mut cut = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if used + w + 1 > max {
            break;
        }
        cut.push(c);
        used += w;
    }
    if max > 0 {
        cut.push('…');
    }
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_field_name_reads_as_words() {
        assert_eq!(words("instanceType"), "instance type");
        assert_eq!(words("name"), "name");
        assert_eq!(words("ipv4Address"), "ipv4 address");
        assert_eq!(words("vpc_id"), "vpc id");
        assert_eq!(words("imageFamilies"), "image families");
        assert_eq!(header("createTime"), "CREATE TIME");
    }

    #[test]
    fn where_a_list_looked_reads_as_words() {
        let scope =
            |parents: &[&str]| scope(&parents.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>());
        assert_eq!(
            scope(&["projects/acme/zones/z1"]).as_deref(),
            Some("in project acme, zone z1")
        );
        assert_eq!(
            scope(&["projects/acme/zones/z1", "projects/acme/zones/z2"]).as_deref(),
            Some("in any zone of project acme")
        );
        assert_eq!(
            scope(&["projects/p/regions/r/dnsPolicies/d"]).as_deref(),
            Some("in project p, region r, dns policy d")
        );
        assert_eq!(scope(&[]), None);
        assert_eq!(scope(&[""]), None);
        assert_eq!(scope(&["projects"]), None);
    }

    #[test]
    fn a_resource_name_is_shown_by_its_last_segment() {
        assert_eq!(
            short_name("projects/acme/zones/eu-central1-az1/instances/web-1"),
            "web-1"
        );
        assert_eq!(short_name("web-1"), "web-1");
    }

    #[test]
    fn a_scalar_or_a_list_of_them_is_text_and_nothing_else_is() {
        assert_eq!(scalar(&json!("a")).as_deref(), Some("a"));
        assert_eq!(scalar(&json!(3)).as_deref(), Some("3"));
        assert_eq!(scalar(&json!(false)).as_deref(), Some("false"));
        assert_eq!(scalar(&json!(["a", "b"])).as_deref(), Some("a, b"));
        assert_eq!(scalar(&json!({"a": 1})), None);
        assert_eq!(scalar(&json!([{"a": 1}])), None);
        assert_eq!(scalar(&json!([["a"]])), None);
        assert_eq!(scalar(&Value::Null), None);
    }

    #[test]
    fn control_characters_from_the_api_reach_the_terminal_escaped() {
        assert_eq!(one_line("a\nb\tc"), "a b c");
        assert_eq!(one_line("\u{1b}]0;owned\u{7}"), "\\u{1b}]0;owned\\u{7}");
        assert_eq!(lines("a\nb\u{1b}"), "a\nb\\u{1b}");
    }

    #[test]
    fn a_cut_value_ends_in_an_ellipsis_within_its_width() {
        assert_eq!(truncate("abcdef", 6), "abcdef");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(width(&truncate("日本語テキスト", 5)), 5);
        assert_eq!(truncate("abc", 0), "");
    }
}
