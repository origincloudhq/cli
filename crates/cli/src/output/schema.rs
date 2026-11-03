//! What the API's descriptors say a JSON field holds, which decides how the
//! human formats show it: a timestamp as how long ago it was, a reference
//! by its resource's id, a state in its colour, a field left out as its
//! default. An output without descriptors, such as a hand-written
//! command's, has its kinds told from its values instead.

use prost_reflect::{FieldDescriptor, Kind as Proto, MessageDescriptor};
use serde_json::Value;

/// What a field holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Kind {
    Bool,
    Number,
    Text,
    /// An enum, which JSON writes as its value's name: `RUNNING`.
    Enum,
    /// A `google.protobuf.Timestamp`, which JSON writes in RFC 3339.
    Timestamp,
    /// A `google.protobuf.Duration`, which JSON writes as `30s`.
    Duration,
    /// The name of another resource (`google.api.resource_reference`).
    Reference,
    /// A `map<string, string>`, or any other map.
    Map,
    /// A message, by its descriptor when known.
    Message(Option<MessageDescriptor>),
}

/// A field's kind, and whether it is a list of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Field {
    pub kind: Kind,
    pub repeated: bool,
}

impl Field {
    pub fn of(field: &FieldDescriptor) -> Self {
        if field.is_map() {
            return Field {
                kind: Kind::Map,
                repeated: false,
            };
        }
        let kind = match field.kind() {
            Proto::Bool => Kind::Bool,
            Proto::Enum(_) => Kind::Enum,
            Proto::String if is_reference(field) => Kind::Reference,
            Proto::String | Proto::Bytes => Kind::Text,
            Proto::Message(message) => message_kind(message),
            _ => Kind::Number,
        };
        Field {
            kind,
            repeated: field.is_list(),
        }
    }

    /// The kind of a field without descriptors, from a value it holds.
    pub fn told_from(value: &Value) -> Self {
        let (value, repeated) = match value {
            Value::Array(items) => (items.first().unwrap_or(&Value::Null), true),
            value => (value, false),
        };
        let kind = match value {
            Value::Bool(_) => Kind::Bool,
            Value::Number(_) => Kind::Number,
            Value::Object(_) => Kind::Message(None),
            _ => Kind::Text,
        };
        Field { kind, repeated }
    }

    /// Whether a table cell can hold it: anything but a message, or a list
    /// of them. A map can, as `KEY=VALUE` pairs.
    pub fn fits_in_a_cell(&self) -> bool {
        !matches!(self.kind, Kind::Message(_))
    }

    /// What a cell shows for an item that leaves the field out. The API's
    /// JSON leaves out a field at its default, so that an item without
    /// `available` is one that isn't: its cell says `false`, not nothing.
    pub fn left_out(&self) -> &'static str {
        match (&self.kind, self.repeated) {
            (Kind::Bool, false) => "false",
            (Kind::Number, false) => "0",
            _ => "",
        }
    }

    /// The message an item of the field is, when it is one.
    pub fn message(&self) -> Option<&MessageDescriptor> {
        match &self.kind {
            Kind::Message(message) => message.as_ref(),
            _ => None,
        }
    }
}

fn message_kind(message: MessageDescriptor) -> Kind {
    match message.full_name() {
        "google.protobuf.Timestamp" => Kind::Timestamp,
        "google.protobuf.Duration" => Kind::Duration,
        "google.protobuf.FieldMask"
        | "google.protobuf.StringValue"
        | "google.protobuf.BytesValue" => Kind::Text,
        "google.protobuf.BoolValue" => Kind::Bool,
        "google.protobuf.Int32Value"
        | "google.protobuf.Int64Value"
        | "google.protobuf.UInt32Value"
        | "google.protobuf.UInt64Value"
        | "google.protobuf.FloatValue"
        | "google.protobuf.DoubleValue" => Kind::Number,
        // Their JSON is any JSON: what it holds tells what it is.
        "google.protobuf.Struct" | "google.protobuf.Value" | "google.protobuf.ListValue" => {
            Kind::Message(None)
        }
        _ => Kind::Message(Some(message)),
    }
}

fn is_reference(field: &FieldDescriptor) -> bool {
    field
        .parent_pool()
        .get_extension_by_name("google.api.resource_reference")
        .is_some_and(|ext| field.options().has_extension(&ext))
}

/// The field of `message` its JSON names `key`.
pub(super) fn field(message: &MessageDescriptor, key: &str) -> Option<FieldDescriptor> {
    message
        .fields()
        .find(|f| f.json_name() == key)
        .or_else(|| message.get_field_by_name(key))
}

/// The message an `Any`'s JSON holds, from its `@type`
/// (`type.googleapis.com/origincloud.pvs.v1beta1.Volume`).
pub(super) fn any_message(fields: &serde_json::Map<String, Value>) -> Option<MessageDescriptor> {
    let url = fields.get("@type")?.as_str()?;
    let name = url.rsplit('/').next()?;
    crate::descriptors::message(name)
}

/// The kind of the field `path` reaches from `message`, through messages
/// and lists of them: repeated when any step is.
pub(super) fn path(message: &MessageDescriptor, path: &[&str]) -> Option<Field> {
    let mut message = message.clone();
    let mut repeated = false;
    let (last, steps) = path.split_last()?;
    for step in steps {
        let field = field(&message, step)?;
        repeated |= field.is_list();
        message = match field.kind() {
            Proto::Message(m) if !field.is_map() => m,
            _ => return None,
        };
    }
    let mut leaf = Field::of(&field(&message, last)?);
    leaf.repeated |= repeated;
    Some(leaf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(name: &str) -> MessageDescriptor {
        crate::descriptors::message(name).unwrap()
    }

    #[test]
    fn the_descriptors_tell_each_field_s_kind() {
        let instance = message("origincloud.compute.v1beta1.Instance");
        let kind = |key: &str| Field::of(&field(&instance, key).unwrap());
        assert_eq!(kind("name").kind, Kind::Text);
        assert_eq!(kind("state").kind, Kind::Enum);
        assert_eq!(kind("createTime").kind, Kind::Timestamp);
        assert_eq!(kind("instanceType").kind, Kind::Reference);
        assert_eq!(
            kind("networkInterfaces"),
            Field {
                kind: Kind::Reference,
                repeated: true
            }
        );
        assert_eq!(kind("tags").kind, Kind::Map);
        assert!(kind("bootVolume").message().is_some());
        assert!(!kind("bootVolume").fits_in_a_cell());
    }

    #[test]
    fn a_path_reaches_through_messages_and_lists_of_them() {
        let instance = message("origincloud.compute.v1beta1.Instance");
        assert_eq!(
            path(&instance, &["interfaceAddresses", "ipv4Address"]),
            Some(Field {
                kind: Kind::Text,
                repeated: true
            })
        );
        assert_eq!(
            path(&instance, &["bootVolume", "sizeGib"]),
            Some(Field {
                kind: Kind::Number,
                repeated: false
            })
        );
        assert_eq!(path(&instance, &["bootVolume", "nothing"]), None);
        assert_eq!(path(&instance, &["state", "x"]), None);
    }

    #[test]
    fn a_left_out_field_shows_its_default() {
        let plain = |kind| Field {
            kind,
            repeated: false,
        };
        assert_eq!(plain(Kind::Bool).left_out(), "false");
        assert_eq!(plain(Kind::Number).left_out(), "0");
        assert_eq!(plain(Kind::Enum).left_out(), "");
        assert_eq!(
            Field {
                kind: Kind::Number,
                repeated: true
            }
            .left_out(),
            ""
        );
    }
}
