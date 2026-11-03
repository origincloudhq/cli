//! How a field's value shows, in a table's cell or on a resource's line,
//! by what the field holds.

use anstyle::Style;
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::schema::{Field, Kind};
use super::{style, text};

/// Where a value is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Place {
    /// A cell of `table`: as short as it can be told apart by.
    Table,
    /// A cell of `wide`: as `table`, but every item of a list.
    Wide,
    /// A resource's line: whole.
    Detail,
}

/// A value as text, and its style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Shown {
    pub text: String,
    pub style: Option<Style>,
}

impl Shown {
    fn plain(text: String) -> Self {
        Shown { text, style: None }
    }
}

/// How many items of a list a `table` cell shows before saying how many
/// more there are.
const MOST_ITEMS: usize = 3;

/// `value`, which `field` holds, as text at `place`; `None` for what
/// doesn't go on one line, a message.
///
/// - a timestamp: how long ago it is (`3d ago`) in a table; on a
///   resource's line, as the API sends it, and how long ago;
/// - a reference: its resource's id (the last segment of its name) in a
///   table; whole on a resource's line;
/// - a state: in its colour (see [`state_style`]);
/// - a map: its `KEY=VALUE` pairs in a table (a resource's lines show it
///   as a block of its own);
/// - a list: its items, joined with `, `; in `table`, the first
///   [`MOST_ITEMS`] and how many more.
pub(super) fn show(
    value: &Value,
    field: &Field,
    place: Place,
    now: DateTime<Utc>,
) -> Option<Shown> {
    match value {
        Value::Array(items) if field.repeated => {
            let shown = items
                .iter()
                .map(|item| one(item, &field.kind, place, now).map(|s| s.text))
                .collect::<Option<Vec<_>>>()?;
            let more = shown.len().saturating_sub(MOST_ITEMS);
            if place == Place::Table && more > 0 {
                let mut first = shown[..MOST_ITEMS].to_vec();
                first.push(format!("+{more}"));
                return Some(Shown::plain(first.join(", ")));
            }
            Some(Shown::plain(shown.join(", ")))
        }
        value => one(value, &field.kind, place, now),
    }
}

fn one(value: &Value, kind: &Kind, place: Place, now: DateTime<Utc>) -> Option<Shown> {
    match (kind, value) {
        // On a resource's line, a map is a block of its pairs instead.
        (Kind::Map, Value::Object(pairs)) if place != Place::Detail => {
            let pairs: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("{k}={}", text::scalar(v).unwrap_or_else(|| v.to_string())))
                .collect();
            Some(Shown::plain(pairs.join(", ")))
        }
        (Kind::Message(_), _) | (_, Value::Object(_)) => None,
        (Kind::Timestamp, Value::String(raw)) => {
            let Ok(time) = DateTime::parse_from_rfc3339(raw) else {
                return Some(Shown::plain(raw.clone()));
            };
            let ago = ago(time.with_timezone(&Utc), now);
            Some(Shown::plain(match place {
                Place::Detail => format!("{raw} ({ago})"),
                Place::Table | Place::Wide => ago,
            }))
        }
        (Kind::Reference, Value::String(name)) if place != Place::Detail => {
            Some(Shown::plain(text::short_name(name).to_owned()))
        }
        (Kind::Enum, Value::String(state)) => Some(Shown {
            text: state.clone(),
            style: state_style(state),
        }),
        (_, value) => text::scalar(value).map(Shown::plain),
    }
}

/// How long ago `time` was, before `now`, as `kubectl` says it: in
/// seconds up to two minutes (`45s ago`), minutes up to two hours (`90m
/// ago`), hours up to two days, days up to two years (`3d ago`), then
/// years; `in 3d` when it is to come.
pub(super) fn ago(time: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let seconds = (now - time).num_seconds();
    let span = seconds.unsigned_abs();
    let span = match span {
        s if s < 120 => format!("{s}s"),
        s if s < 2 * 3600 => format!("{}m", s / 60),
        s if s < 48 * 3600 => format!("{}h", s / 3600),
        s if s < 730 * 86400 => format!("{}d", s / 86400),
        s => format!("{}y", s / (365 * 86400)),
    };
    if seconds < 0 {
        format!("in {span}")
    } else {
        format!("{span} ago")
    }
}

/// The colour of a state, by its name, so that what needs a look stands
/// out: red for what failed, green for what is up and in use, yellow for
/// what is on its way to another state (`STARTING`, `PENDING`), dim for
/// what is stopped or gone. The name is always shown: the colour only adds
/// to it.
pub(super) fn state_style(state: &str) -> Option<Style> {
    const BAD: [&str; 5] = [
        "FAILED",
        "ERROR",
        "REJECTED",
        "IMPAIRED",
        "RELEASE_ABANDONED",
    ];
    const GOOD: [&str; 9] = [
        "RUNNING",
        "AVAILABLE",
        "ATTACHED",
        "IN_USE",
        "COMPLETED",
        "ACTIVE",
        "PROTECTED",
        "RESIZE_COMPLETED",
        "OK",
    ];
    const GONE: [&str; 3] = ["STOPPED", "TERMINATED", "DELETED"];
    if BAD.contains(&state) || state.ends_with("_FAILED") {
        Some(style::BAD)
    } else if GOOD.contains(&state) {
        Some(style::GOOD)
    } else if GONE.contains(&state) {
        Some(style::GONE)
    } else if state.ends_with("ING") || state.starts_with("PENDING") {
        Some(style::CHANGING)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        "2026-11-03T12:00:00Z".parse().unwrap()
    }

    fn field(kind: Kind, repeated: bool) -> Field {
        Field { kind, repeated }
    }

    fn shown(value: Value, kind: Kind, repeated: bool, place: Place) -> String {
        show(&value, &field(kind, repeated), place, now())
            .unwrap()
            .text
    }

    #[test]
    fn a_timestamp_is_how_long_ago_in_a_table_and_both_on_a_line() {
        let time = json!("2026-10-31T09:12:00Z");
        assert_eq!(
            shown(time.clone(), Kind::Timestamp, false, Place::Table),
            "3d ago"
        );
        assert_eq!(
            shown(time, Kind::Timestamp, false, Place::Detail),
            "2026-10-31T09:12:00Z (3d ago)"
        );
        // Not one: as it is.
        assert_eq!(
            shown(json!("soon"), Kind::Timestamp, false, Place::Table),
            "soon"
        );
    }

    #[test]
    fn how_long_ago_is_in_its_largest_whole_unit() {
        let at = |s: &str| s.parse::<DateTime<Utc>>().unwrap();
        assert_eq!(ago(at("2026-11-03T11:59:15Z"), now()), "45s ago");
        assert_eq!(ago(at("2026-11-03T11:48:00Z"), now()), "12m ago");
        assert_eq!(ago(at("2026-11-03T05:00:00Z"), now()), "7h ago");
        assert_eq!(ago(at("2024-10-01T00:00:00Z"), now()), "2y ago");
        assert_eq!(ago(at("2026-11-06T12:00:00Z"), now()), "in 3d");
    }

    #[test]
    fn a_reference_is_its_id_in_a_table_and_whole_on_a_line() {
        let vpc = json!("projects/p/regions/r/vpcs/main");
        assert_eq!(
            shown(vpc.clone(), Kind::Reference, false, Place::Table),
            "main"
        );
        assert_eq!(
            shown(vpc.clone(), Kind::Reference, false, Place::Wide),
            "main"
        );
        assert_eq!(
            shown(vpc, Kind::Reference, false, Place::Detail),
            "projects/p/regions/r/vpcs/main"
        );
    }

    #[test]
    fn a_table_shows_a_list_s_first_items_and_how_many_more() {
        let zones = json!(["z1", "z2", "z3", "z4", "z5"]);
        assert_eq!(
            shown(zones.clone(), Kind::Text, true, Place::Table),
            "z1, z2, z3, +2"
        );
        assert_eq!(
            shown(zones, Kind::Text, true, Place::Wide),
            "z1, z2, z3, z4, z5"
        );
    }

    #[test]
    fn a_map_is_its_pairs_and_a_message_isn_t_a_line() {
        let tags = json!({"env": "prod", "team": "web"});
        assert_eq!(
            shown(tags.clone(), Kind::Map, false, Place::Table),
            "env=prod, team=web"
        );
        assert_eq!(
            show(
                &tags,
                &field(Kind::Message(None), false),
                Place::Table,
                now()
            ),
            None
        );
    }

    #[test]
    fn a_state_is_in_its_colour() {
        let state =
            |s: &str| show(&json!(s), &field(Kind::Enum, false), Place::Table, now()).unwrap();
        assert_eq!(state("RUNNING").style, Some(style::GOOD));
        assert_eq!(state("FAILED").style, Some(style::BAD));
        assert_eq!(state("RESIZE_FAILED").style, Some(style::BAD));
        assert_eq!(state("STARTING").style, Some(style::CHANGING));
        assert_eq!(state("PENDING_REVIEW").style, Some(style::CHANGING));
        assert_eq!(state("STOPPED").style, Some(style::GONE));
        assert_eq!(state("TCP").style, None);
        assert_eq!(state("RUNNING").text, "RUNNING");
        // Text that happens to read like a state isn't coloured.
        let text = show(
            &json!("FAILED"),
            &field(Kind::Text, false),
            Place::Table,
            now(),
        )
        .unwrap();
        assert_eq!(text.style, None);
    }
}
