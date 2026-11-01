//! What the generated flags of nested messages, JSON values and timestamps
//! use: the `KEY=VALUE,...` form and its typed keys, the `--{field}-json`
//! form checked against the API's descriptors, and the notices an update
//! gives when it replaces a field whole.
//!
//! The published API has no nested message yet, so only the tests' scratch
//! service (`test-services`) uses most of this today.

#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::LazyLock;

use prost_reflect::{DescriptorPool, FieldDescriptor, Kind, MessageDescriptor, Value};
use serde::de::{DeserializeOwned, DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor};

use super::runtime::duration;

/// A nested message from its `KEY=VALUE,...` form, with the keys given, so
/// that an update can say which it resets.
#[derive(Clone, Debug)]
pub struct Keyed<T> {
    pub value: T,
    /// The keys given, as the message's field names.
    pub given: Vec<String>,
}

/// A message field's value from its JSON flag (`--rules-json`), with the
/// fields an object gave, by their names in the definitions.
#[derive(Clone, Debug)]
pub struct JsonArg<T>(pub T, pub Vec<String>);

/// Reads a JSON flag's value: `value` itself, or, as `@FILE`, the file's
/// content, the JSON of the message `message` or a list of them. It is
/// checked against the message's descriptor at every depth, each refusal
/// naming its path: a key given twice, a key that is no field's, a value of
/// the wrong type, an enum value that isn't one, an element given twice in
/// a list whose order the API doesn't keep, or an id where a whole resource
/// name is needed. The SDK's decoder would drop or default most of these,
/// and the request go without them.
pub fn json_arg<T: DeserializeOwned>(value: &str, message: &str) -> Result<JsonArg<T>, String> {
    let text = match value.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))?,
        None => value.to_owned(),
    };
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    NoDuplicates(String::new())
        .deserialize(&mut deserializer)
        .map_err(|e| {
            let message = e.to_string();
            if message.contains(" is given twice") {
                message
            } else {
                format!("not JSON: {message}")
            }
        })?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("not JSON: {e}"))?;
    let descriptor =
        message_descriptor(message).ok_or_else(|| format!("no definition of {message}"))?;
    let mut given = Vec::new();
    // A well-known type has a JSON form of its own: a struct is any object.
    let own_form = descriptor.package_name() == "google.protobuf";
    match &json {
        _ if own_form => {}
        serde_json::Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                check_message(item, &descriptor, &format!("[{i}]"), 0)?;
            }
        }
        other => {
            check_message(other, &descriptor, "", 0)?;
            if let serde_json::Value::Object(object) = other {
                given = object
                    .keys()
                    .filter_map(|k| field_of(&descriptor, k).map(|f| f.name().to_owned()))
                    .collect();
            }
        }
    }
    serde_json::from_value(json)
        .map(|value| JsonArg(value, given))
        .map_err(|e| format!("not its JSON: {e}"))
}

/// The descriptors of the API's messages, which the generator checked in.
static DESCRIPTORS: LazyLock<Vec<DescriptorPool>> = LazyLock::new(|| {
    #[allow(unused_mut)]
    let mut pools = vec![
        DescriptorPool::decode(include_bytes!("generated/api.binpb").as_slice())
            .expect("the checked-in descriptors decode"),
    ];
    #[cfg(feature = "test-services")]
    pools.push(
        DescriptorPool::decode(include_bytes!("generated/test-api.binpb").as_slice())
            .expect("the checked-in descriptors decode"),
    );
    pools
});

fn message_descriptor(name: &str) -> Option<MessageDescriptor> {
    DESCRIPTORS
        .iter()
        .find_map(|pool| pool.get_message_by_name(name))
}

/// The field `key` names: by its name in the definitions, or its JSON name,
/// as the API's JSON parser accepts either.
fn field_of(message: &MessageDescriptor, key: &str) -> Option<FieldDescriptor> {
    message
        .get_field_by_name(key)
        .or_else(|| message.get_field_by_json_name(key))
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

/// `google.api.FieldBehavior.UNORDERED_LIST`.
const UNORDERED_LIST: i32 = 6;

/// Whether `field` has the field behavior `behavior`.
fn has_behavior(field: &FieldDescriptor, behavior: i32) -> bool {
    let Some(ext) = field
        .parent_pool()
        .get_extension_by_name("google.api.field_behavior")
    else {
        return false;
    };
    let options = field.options();
    if !options.has_extension(&ext) {
        return false;
    }
    match options.get_extension(&ext).as_ref() {
        Value::List(values) => values.iter().any(|v| v.as_enum_number() == Some(behavior)),
        _ => false,
    }
}

/// Whether `field` holds a resource name, by its `google.api.resource_reference`.
fn is_reference(field: &FieldDescriptor) -> bool {
    let Some(ext) = field
        .parent_pool()
        .get_extension_by_name("google.api.resource_reference")
    else {
        return false;
    };
    field.options().has_extension(&ext)
}

/// Checks the object `value` against `message`, at `depth` messages below
/// the flag's own.
fn check_message(
    value: &serde_json::Value,
    message: &MessageDescriptor,
    path: &str,
    depth: usize,
) -> Result<(), String> {
    let serde_json::Value::Object(object) = value else {
        return Err(format!("{}: {value} isn't an object", at(path)));
    };
    let mut seen: Vec<(u32, &str)> = Vec::new();
    let mut chosen: Vec<(String, &str)> = Vec::new();
    for (key, value) in object {
        let here = join(path, key);
        let field = field_of(message, key)
            .ok_or_else(|| format!("{here} isn't a field of {}", message.name()))?;
        if let Some((_, before)) = seen.iter().find(|(n, _)| *n == field.number()) {
            return Err(format!("{here} is {} again", join(path, before)));
        }
        seen.push((field.number(), key));
        // At most one member of a oneof; `null` sets none.
        if let Some(oneof) = field.containing_oneof().filter(|o| !o.is_synthetic()) {
            if !value.is_null() {
                if let Some((_, other)) = chosen.iter().find(|(o, _)| o == oneof.name()) {
                    return Err(format!(
                        "{here} and {} are both of {}'s choice {}: give one",
                        join(path, other),
                        message.name(),
                        oneof.name()
                    ));
                }
                chosen.push((oneof.name().to_owned(), key));
            }
        }
        check_field(value, &field, &here, depth)?;
    }
    Ok(())
}

/// The path of a value for a refusal: the flag's value itself when empty.
fn at(path: &str) -> &str {
    if path.is_empty() {
        "the value"
    } else {
        path
    }
}

fn check_field(
    value: &serde_json::Value,
    field: &FieldDescriptor,
    here: &str,
    depth: usize,
) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    if field.is_map() {
        let serde_json::Value::Object(entries) = value else {
            return Err(format!("{here}: {value} isn't an object"));
        };
        let Kind::Message(entry) = field.kind() else {
            return Ok(());
        };
        let value_field = entry.map_entry_value_field();
        for (k, v) in entries {
            check_value(v, &value_field, &format!("{here}[{k:?}]"), depth + 1, false)?;
        }
        return Ok(());
    }
    if field.is_list() {
        let serde_json::Value::Array(items) = value else {
            return Err(format!("{here}: {value} isn't a list"));
        };
        for (i, item) in items.iter().enumerate() {
            check_value(item, field, &format!("{here}[{i}]"), depth, true)?;
        }
        if has_behavior(field, UNORDERED_LIST) {
            for (i, item) in items.iter().enumerate() {
                if let Some(j) = items[..i].iter().position(|other| other == item) {
                    return Err(format!(
                        "{here}[{i}] is {here}[{j}] again; the API keeps each element once"
                    ));
                }
            }
        }
        return Ok(());
    }
    check_value(value, field, here, depth, false)
}

/// Checks one value of `field`'s type: an element, for a list.
fn check_value(
    value: &serde_json::Value,
    field: &FieldDescriptor,
    here: &str,
    depth: usize,
    element: bool,
) -> Result<(), String> {
    use serde_json::Value as J;
    match field.kind() {
        Kind::Message(m) => {
            // The well-known types have JSON forms of their own: a duration
            // or a timestamp is a string, a struct any object.
            if m.package_name() == "google.protobuf" {
                return Ok(());
            }
            check_message(value, &m, here, depth + 1)
        }
        Kind::Enum(e) => {
            let names: Vec<String> = e
                .values()
                .filter(|v| v.number() != 0)
                .map(|v| v.name().to_owned())
                .collect();
            let known = match value {
                J::String(s) => e.get_value_by_name(s).is_some_and(|v| v.number() != 0),
                J::Number(n) => n
                    .as_i64()
                    .and_then(|n| i32::try_from(n).ok())
                    .and_then(|n| e.get_value(n))
                    .is_some_and(|v| v.number() != 0),
                _ => false,
            };
            if known {
                Ok(())
            } else {
                Err(format!("{here}: {value} isn't one of {}", names.join(", ")))
            }
        }
        Kind::Bool => match value {
            J::Bool(_) => Ok(()),
            _ => Err(format!("{here}: {value} isn't true or false")),
        },
        Kind::String => match value {
            J::String(s) => {
                // A resource name a flag's own keys don't give, which nothing
                // completes: a singular one of the flag's message is
                // completed with `--project` and `--zone`, any other must be
                // whole.
                let completed = depth == 0 && !element;
                if is_reference(field) && !completed && !s.is_empty() && !s.contains('/') {
                    Err(format!(
                        "{here}: {s:?} is an id; give the resource's whole name here"
                    ))
                } else {
                    Ok(())
                }
            }
            _ => Err(format!("{here}: {value} isn't a string")),
        },
        Kind::Bytes => match value {
            J::String(_) => Ok(()),
            _ => Err(format!("{here}: {value} isn't a base64 string")),
        },
        Kind::Float | Kind::Double => {
            let number = match value {
                J::Number(_) => true,
                J::String(s) => {
                    matches!(s.as_str(), "NaN" | "Infinity" | "-Infinity")
                        || s.parse::<f64>().is_ok()
                }
                _ => false,
            };
            if number {
                Ok(())
            } else {
                Err(format!("{here}: {value} isn't a number"))
            }
        }
        kind => {
            let (min, max): (i128, i128) = match kind {
                Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => (i32::MIN.into(), i32::MAX.into()),
                Kind::Uint32 | Kind::Fixed32 => (0, u32::MAX.into()),
                Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => (i64::MIN.into(), i64::MAX.into()),
                _ => (0, u64::MAX.into()),
            };
            let whole = match value {
                J::Number(n) => n
                    .as_i64()
                    .map(i128::from)
                    .or_else(|| n.as_u64().map(i128::from)),
                J::String(s) => s.parse::<i128>().ok(),
                _ => None,
            };
            match whole {
                Some(n) if (min..=max).contains(&n) => Ok(()),
                _ => Err(format!(
                    "{here}: {value} isn't a whole number from {min} to {max}"
                )),
            }
        }
    }
}

/// A JSON value with no object giving a key twice, which `serde_json`
/// would read as its last.
struct NoDuplicates(String);

impl<'de> DeserializeSeed<'de> for NoDuplicates {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for NoDuplicates {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JSON")
    }

    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E>(self, _: &str) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        let mut i = 0;
        while seq
            .next_element_seed(NoDuplicates(format!("{}[{i}]", self.0)))?
            .is_some()
        {
            i += 1;
        }
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut seen = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            let here = join(&self.0, &key);
            if !seen.insert(key) {
                return Err(A::Error::custom(format!("{here} is given twice")));
            }
            map.next_value_seed(NoDuplicates(here))?;
        }
        Ok(())
    }
}

/// The pairs of a nested message's `KEY=VALUE,...` form, in order, each key
/// as its field's name: `idle-timeout` is `idle_timeout`. A key is given at
/// most once; a value runs to the next comma, so one holding a comma goes
/// through the JSON flag.
pub fn key_values(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for part in text.split(',') {
        let (key, value) = part
            .split_once('=')
            .filter(|(k, _)| !k.trim().is_empty())
            .ok_or_else(|| format!("{part:?} isn't KEY=VALUE"))?;
        let key = key.trim().replace('-', "_");
        if pairs.iter().any(|(k, _)| *k == key) {
            return Err(format!("{key} is given twice"));
        }
        pairs.push((key, value.to_owned()));
    }
    Ok(pairs)
}

/// A key's number.
pub fn key_scalar<T>(key: &str, value: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|e| format!("{key}: {value:?} isn't a number: {e}"))
}

/// A key's boolean, `true` or `false`.
pub fn key_bool(key: &str, value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("{key}: {value:?} isn't true or false")),
    }
}

/// A key's enum value, one of `names`.
pub fn key_enum(
    key: &str,
    value: &str,
    names: &[&str],
    parse: impl Fn(&str) -> Option<i32>,
) -> Result<i32, String> {
    parse(value)
        .filter(|_| names.contains(&value))
        .ok_or_else(|| format!("{key}: {value:?} isn't one of {}", names.join(", ")))
}

/// A key's duration, such as `30s`.
pub fn key_duration(
    key: &str,
    value: &str,
) -> Result<origincloud_core::protobuf::Duration, String> {
    duration(value).map_err(|e| format!("{key}: {e}"))
}

/// A timestamp flag's value: RFC 3339, such as `2026-11-01T12:00:00Z`.
pub fn timestamp(value: &str) -> Result<origincloud_core::protobuf::Timestamp, String> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|t| t.with_timezone(&chrono::Utc).into())
        .map_err(|e| format!("{value:?} isn't an RFC 3339 time, such as 2026-11-01T12:00:00Z: {e}"))
}

/// A key's timestamp.
pub fn key_timestamp(
    key: &str,
    value: &str,
) -> Result<origincloud_core::protobuf::Timestamp, String> {
    timestamp(value).map_err(|e| format!("{key}: {e}"))
}

/// What a key that isn't one says: the keys there are, or that the field is
/// one only `json_flag` sets.
pub fn unknown_key(key: &str, keys: &[&str], json_only: &[&str], json_flag: &str) -> String {
    if json_only.contains(&key) {
        format!("{key} is set only through {json_flag}")
    } else {
        format!("{key} isn't a key; the keys are {}", keys.join(", "))
    }
}

/// Checks every required key is among `pairs`.
pub fn required_keys(pairs: &[(String, String)], required: &[&str]) -> Result<(), String> {
    match required
        .iter()
        .find(|r| !pairs.iter().any(|(k, _)| k == *r))
    {
        Some(missing) => Err(format!("{missing} is required")),
        None => Ok(()),
    }
}

/// Checks no element of a list whose order the API doesn't keep is given
/// twice: the API keeps each once.
pub fn unique<T: PartialEq>(items: &[T], flag: &str) -> anyhow::Result<()> {
    for (i, item) in items.iter().enumerate() {
        if let Some(j) = items[..i].iter().position(|other| other == item) {
            anyhow::bail!(
                "{flag}: the {} value is the {} again; the API keeps each once",
                ordinal(i + 1),
                ordinal(j + 1)
            );
        }
    }
    Ok(())
}

fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// Says on standard error which keys of the nested message `field` an
/// update's `flag` leaves out: the update replaces the field whole, so it
/// resets them.
pub fn notice_reset(flag: &str, field: &str, keys: &[&str], given: &[String]) {
    let missing: Vec<&str> = keys
        .iter()
        .copied()
        .filter(|k| !given.iter().any(|g| g == k))
        .collect();
    if !missing.is_empty() {
        eprintln!(
            "{flag} leaves out {}: the update replaces {field} whole, so it resets them.",
            missing.join(", ")
        );
    }
}

/// Says on standard error that an update replaces every element of the list
/// `field` with the `count` its `flag` gives.
pub fn notice_replaced(flag: &str, field: &str, count: usize) {
    eprintln!("{flag}: the update replaces all of {field} with the {count} given.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nested_message_is_key_value_pairs() {
        assert_eq!(
            key_values("protocol=TCP,cidr=10.0.0.0/8,note=a=b,idle-timeout=1m").unwrap(),
            [
                ("protocol".to_owned(), "TCP".to_owned()),
                ("cidr".to_owned(), "10.0.0.0/8".to_owned()),
                ("note".to_owned(), "a=b".to_owned()),
                ("idle_timeout".to_owned(), "1m".to_owned()),
            ]
        );
        assert_eq!(
            key_values("port=").unwrap(),
            [("port".to_owned(), String::new())]
        );
        for bad in [
            "",
            "port",
            "=1",
            "port=1,",
            "port=1,port=2",
            "idle-timeout=1s,idle_timeout=2s",
        ] {
            assert!(key_values(bad).is_err(), "{bad:?}");
        }
        assert_eq!(key_scalar::<i32>("port", "443"), Ok(443));
        assert!(key_scalar::<i32>("port", "https")
            .unwrap_err()
            .starts_with("port:"));
        assert_eq!(key_bool("allow", "true"), Ok(true));
        assert!(key_bool("allow", "yes").is_err());
        let parse = |v: &str| match v {
            "TCP" => Some(1),
            "PROTOCOL_UNSPECIFIED" => Some(0),
            _ => None,
        };
        assert_eq!(key_enum("protocol", "TCP", &["TCP"], parse), Ok(1));
        assert!(key_enum("protocol", "PROTOCOL_UNSPECIFIED", &["TCP"], parse).is_err());
        assert_eq!(key_duration("idle_timeout", "1m").unwrap().seconds, 60);
        assert_eq!(
            unknown_key("match", &["port"], &["match"], "--rules-json"),
            "match is set only through --rules-json"
        );
        assert!(unknown_key("prot", &["port"], &[], "--rules-json").contains("the keys are port"));
        let pairs = key_values("port=1").unwrap();
        assert!(required_keys(&pairs, &["port"]).is_ok());
        assert_eq!(
            required_keys(&pairs, &["protocol"]),
            Err("protocol is required".to_owned())
        );
    }

    #[test]
    fn a_timestamp_is_rfc_3339() {
        let t = timestamp("2026-11-01T12:00:00.5+01:00").unwrap();
        assert_eq!((t.seconds, t.nanos), (1_793_530_800, 500_000_000));
        for bad in ["2026-11-01", "yesterday", "2026-11-01 12:00:00"] {
            assert!(timestamp(bad).is_err(), "{bad}");
        }
        assert!(key_timestamp("not_after", "x")
            .unwrap_err()
            .starts_with("not_after:"));
    }

    #[test]
    fn a_list_the_api_keeps_unordered_has_each_element_once() {
        assert!(unique(&["a", "b"], "--alias").is_ok());
        assert_eq!(
            unique(&["a", "b", "a"], "--alias").unwrap_err().to_string(),
            "--alias: the 3rd value is the 1st again; the API keeps each once"
        );
    }

    #[test]
    fn a_json_flag_takes_json_or_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.json");
        std::fs::write(&path, r#"{"sizeGib": 8}"#).unwrap();
        let volume = "origincloud.pvs.v1beta1.Volume";
        type Volume = origincloud_pvs::v1beta1::Volume;
        let read: JsonArg<Volume> = json_arg(&format!("@{}", path.display()), volume).unwrap();
        assert_eq!(read.0.size_gib, 8);
        assert_eq!(read.1, ["size_gib"]);
        // Its name in the definitions, or its JSON name.
        assert_eq!(
            json_arg::<Volume>(r#"{"size_gib": 9}"#, volume)
                .unwrap()
                .0
                .size_gib,
            9
        );
        let list = json_arg::<Vec<Volume>>(r#"[{"sizeGib": 1}, {"size_gib": 2}]"#, volume).unwrap();
        assert_eq!(list.0.len(), 2);
        for (json, refusal) in [
            (r#"{"sizeGb": 8}"#, "sizeGb isn't a field of Volume"),
            (
                r#"[{"sizeGib": 1}, {"tag": {}}]"#,
                "[1].tag isn't a field of Volume",
            ),
            (r#"{"sizeGib": "x"}"#, "sizeGib: \"x\" isn't a whole number"),
            (r#"{"sizeGib": 1.5}"#, "sizeGib: 1.5 isn't a whole number"),
            (
                r#"{"sizeGib": 4294967296}"#,
                "sizeGib: 4294967296 isn't a whole number from -2147483648 to 2147483647",
            ),
            (r#"{"state": "MELTED"}"#, "state: \"MELTED\" isn't one of"),
            (r#"{"state": "STATE_UNSPECIFIED"}"#, "isn't one of"),
            (r#"{"state": 99}"#, "state: 99 isn't one of"),
            (r#"{"storageClass": 5}"#, "storageClass: 5 isn't a string"),
            (
                r#"{"sizeGib": 1, "size_gib": 2}"#,
                "size_gib is sizeGib again",
            ),
            (r#"{"sizeGib": 1, "sizeGib": 2}"#, "sizeGib is given twice"),
            (r#"{"tags": {"a": 1}}"#, "tags[\"a\"]: 1 isn't a string"),
            ("[1]", "[0]: 1 isn't an object"),
            ("{", "not JSON: EOF"),
        ] {
            let result = if json.starts_with('[') {
                json_arg::<Vec<Volume>>(json, volume).map(drop)
            } else {
                json_arg::<Volume>(json, volume).map(drop)
            };
            match (refusal, result) {
                ("", _) => {}
                (refusal, Err(e)) => assert!(e.contains(refusal), "{json}: {e}"),
                (_, Ok(())) => panic!("{json} wasn't refused"),
            }
        }
        // A value of an enum, by its name or its number.
        assert!(json_arg::<Volume>(r#"{"state": "AVAILABLE"}"#, volume).is_ok());
        assert!(json_arg::<Volume>(r#"{"sourceSnapshot": "snap-1"}"#, volume).is_ok());
        assert!(json_arg::<Volume>("@/no/such/file", volume)
            .unwrap_err()
            .starts_with("reading"));
    }
}
