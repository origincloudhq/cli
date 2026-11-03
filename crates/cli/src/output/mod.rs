//! What a command prints, and how.
//!
//! Every command's output is JSON first ([`SerializableOutput::to_json`]);
//! the human formats, `table`, `wide` and `name`, are made from that JSON,
//! so that they can't say something `json` doesn't, and so that the
//! hand-written commands have them as the generated ones do. What the
//! API's descriptors say of a response's fields (see [`schema`]) decides
//! how each shows. [`format_for_stdout`] picks the format.

mod detail;
mod format;
mod schema;
mod style;
mod table;
mod text;
mod value;

use chrono::{DateTime, Utc};
use prost_reflect::MessageDescriptor;
use serde_json::{Map, Value};

use crate::config::OutputFormat;
pub(crate) use format::{format_for_stdout, width, ColorWhen};

#[derive(thiserror::Error, Debug)]
pub enum SerializationError {
    #[error("other: {0}")]
    Other(String),
}

pub trait SerializableOutput {
    fn to_json(&self) -> Result<serde_json::Value, SerializationError>;

    fn exit_code(&self) -> i32 {
        0
    }

    /// Plain-text rendering that bypasses the JSON envelope, for commands that
    /// support a `--raw` output mode. Returning `None` (the default) keeps the
    /// existing JSON rendering.
    fn to_raw(&self) -> Option<String> {
        None
    }

    /// What the output is, which the human formats print it as. By
    /// default, what its JSON looks like: see [`Shape::Inferred`].
    fn shape(&self) -> Shape {
        Shape::Inferred
    }
}

/// What an output is, for the human formats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    /// What its JSON looks like: a list is a table; an object holding
    /// nothing but one field is that field, as the GraphQL responses wrap
    /// theirs (`{"iam": {"roles": [...]}}` is the list of roles); any other
    /// object is a resource, printed as its fields.
    Inferred,
    /// A message of the API, by its full name
    /// (`origincloud.pvs.v1beta1.Volume`), printed as its fields.
    Message(&'static str),
    /// A list response.
    List(Listing),
    /// A command that has no result, done, and what it did:
    /// `Deleted volume v1.`
    Done(String),
    /// A command that failed, with what went wrong.
    Failed(String),
}

/// A list response, for the human formats.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    /// The field holding the items, beside the next page's token when
    /// there is one: `instances`.
    pub field: &'static str,
    /// The items' message, in full: `origincloud.compute.v1beta1.Instance`.
    pub item: Option<&'static str>,
    /// The columns of `table`, JSON paths from an item, from
    /// `commands.toml`'s view of the message; empty for its default
    /// columns.
    pub columns: &'static [&'static str],
    /// The parents listed, `projects/acme/zones/eu-central1-az1`, which
    /// say where nothing was found.
    pub parents: Vec<String>,
    /// The command creating an item, which an empty list suggests:
    /// `origin compute create-instance`.
    pub create: Option<&'static str>,
}

#[derive(serde::Serialize)]
#[serde(tag = "code", content = "description", rename_all = "snake_case")]
pub enum CommandResult {
    Success,
    /// The rendered failure description.
    ///
    /// The envelope's `code` stays the fixed literal `"failure"` for
    /// backwards compatibility - it has never been the server's error code,
    /// and scripts parse it. The server's stable code and correlation id are
    /// carried inside the description by the clients' rendering, so they
    /// reach the user without changing the envelope's
    /// shape.
    Failure(String),
}

impl SerializableOutput for CommandResult {
    fn to_json(&self) -> Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|e| SerializationError::Other(e.to_string()))
    }

    fn exit_code(&self) -> i32 {
        match self {
            CommandResult::Success => 0,
            CommandResult::Failure(_) => 1,
        }
    }

    fn shape(&self) -> Shape {
        match self {
            CommandResult::Success => Shape::Done("Done.".to_owned()),
            CommandResult::Failure(description) => Shape::Failed(description.clone()),
        }
    }
}

/// What the human formats are laid out for.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Context {
    /// The columns a table is fitted to, when known.
    pub width: Option<usize>,
    /// The time a timestamp says how long ago it is from.
    pub now: DateTime<Utc>,
}

impl Context {
    /// This terminal's, now.
    pub(crate) fn current() -> Self {
        Context {
            width: width(),
            now: Utc::now(),
        }
    }
}

/// What an output prints, on standard output and on standard error, with
/// the styles of the human formats in it.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Printed {
    pub out: String,
    pub err: String,
}

/// `output` in `format`, laid out for `cx`.
///
/// `json` is what the CLI has always printed, a failure included: its
/// envelope on standard output. The human formats print a failure on
/// standard error as `error: …`, and what a list, a resource or a command
/// without a result is as their help says. A list's next page token, and
/// a list being empty, are said on standard error, so that standard output
/// holds the items alone.
pub(crate) fn render(
    format: OutputFormat,
    output: &dyn SerializableOutput,
    cx: &Context,
) -> anyhow::Result<Printed> {
    if let Some(raw) = output.to_raw() {
        return Ok(Printed {
            out: format!("{raw}\n"),
            err: String::new(),
        });
    }
    if format == OutputFormat::Json {
        let value = output.to_json()?;
        return Ok(Printed {
            out: format!("{}\n", serde_json::to_string_pretty(&value)?),
            err: String::new(),
        });
    }

    let mut printed = Printed::default();
    let shape = output.shape();
    match &shape {
        Shape::Done(what) => {
            if format != OutputFormat::Name {
                let style = style::DONE;
                printed.out = format!("{style}✓{style:#} {}\n", text::one_line(what));
            }
            return Ok(printed);
        }
        Shape::Failed(description) => {
            let style = style::ERROR;
            printed.err = format!("{style}error:{style:#} {}\n", text::lines(description));
            return Ok(printed);
        }
        Shape::Inferred | Shape::Message(_) | Shape::List(_) => {}
    }
    let value = output.to_json()?;

    match what(&shape, &value) {
        What::List {
            items,
            noun,
            next,
            listing,
        } => {
            let item = listing
                .and_then(|l| l.item)
                .and_then(crate::descriptors::message);
            if format == OutputFormat::Name {
                printed.out = names(items.iter())?;
            } else if items.is_empty() {
                printed.err = nothing(noun, listing);
            } else {
                let columns = listing.map_or(&[][..], |l| l.columns);
                let wide = format == OutputFormat::Wide;
                printed.out = list(items, item.as_ref(), columns, wide, cx);
            }
            if let Some(token) = next {
                printed.err.push_str(&format!(
                    "More follow: --page-token {}\n",
                    text::one_line(token)
                ));
            }
        }
        What::One(value, message) => {
            printed.out = if format == OutputFormat::Name {
                names(std::iter::once(value))?
            } else {
                detail::detail(value, message.as_ref(), cx.now)
            };
        }
    }
    Ok(printed)
}

/// What a human format prints of an output's JSON.
enum What<'a> {
    List {
        items: &'a [Value],
        /// The field the items are in, which names them when there are
        /// none: `instanceTypes`.
        noun: Option<&'a str>,
        /// The token of the page after this one.
        next: Option<&'a str>,
        listing: Option<&'a Listing>,
    },
    One(&'a Value, Option<MessageDescriptor>),
}

fn what<'a>(shape: &'a Shape, value: &'a Value) -> What<'a> {
    match shape {
        Shape::List(listing) => {
            let next = value
                .get("nextPageToken")
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty());
            let items = value
                .get(listing.field)
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            return What::List {
                items,
                noun: Some(listing.field),
                next,
                listing: Some(listing),
            };
        }
        Shape::Message(name) => return What::One(value, crate::descriptors::message(name)),
        _ => {}
    }

    // Inferred: unwrapped while it is an object of one field holding an
    // object or a list.
    let mut value = value;
    let mut noun = None;
    loop {
        match value {
            Value::Array(items) => {
                return What::List {
                    items,
                    noun,
                    next: None,
                    listing: None,
                }
            }
            Value::Object(fields) if fields.len() == 1 => {
                let (key, inner) = fields.iter().next().expect("one field");
                if !(inner.is_object() || inner.is_array()) {
                    return What::One(value, None);
                }
                noun = Some(key.as_str());
                value = inner;
            }
            _ => return What::One(value, None),
        }
    }
}

/// What an empty list says: what there is none of, where, and the command
/// making one. `No instances in project acme, zone eu-central1-az1. Create
/// one with `origin compute create-instance`.`
fn nothing(noun: Option<&str>, listing: Option<&Listing>) -> String {
    let what = match noun {
        Some(noun) => text::one_line(&text::words(noun)),
        None => return "Nothing to list.\n".to_owned(),
    };
    let mut said = format!("No {what}");
    if let Some(scope) = listing.and_then(|l| text::scope(&l.parents)) {
        said.push(' ');
        said.push_str(&text::one_line(&scope));
    }
    said.push('.');
    if let Some(create) = listing.and_then(|l| l.create) {
        said.push_str(&format!(" Create one with `{create}`."));
    }
    said.push('\n');
    said
}

/// Items as a table when they are objects, else one per line.
fn list(
    items: &[Value],
    item: Option<&MessageDescriptor>,
    columns: &[&str],
    wide: bool,
    cx: &Context,
) -> String {
    let objects: Option<Vec<&Map<String, Value>>> = items.iter().map(Value::as_object).collect();
    match objects {
        Some(objects) => {
            let specs = table::columns(&objects, item, columns, wide);
            table::table(&objects, &specs, wide, cx.width, cx.now)
        }
        None => items
            .iter()
            .map(|item| {
                let line = text::scalar(item).unwrap_or_else(|| item.to_string());
                format!("{}\n", text::one_line(&line))
            })
            .collect(),
    }
}

/// The `name` of each of `values`, one per line, whole: the form every
/// resource flag takes.
fn names<'a>(values: impl Iterator<Item = &'a Value>) -> anyhow::Result<String> {
    let mut out = String::new();
    for value in values {
        let name = value
            .get("name")
            .and_then(Value::as_str)
            .filter(|n| !n.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "this output has no resource names for `--output name`: use `--output table` \
                     or `--output json`"
                )
            })?;
        out.push_str(&text::one_line(name));
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fixed(Value, Shape);

    impl SerializableOutput for Fixed {
        fn to_json(&self) -> Result<Value, SerializationError> {
            Ok(self.0.clone())
        }

        fn shape(&self) -> Shape {
            self.1.clone()
        }
    }

    fn cx() -> Context {
        Context {
            width: None,
            now: "2026-11-03T12:00:00Z".parse().unwrap(),
        }
    }

    fn printed(format: OutputFormat, output: &dyn SerializableOutput) -> (String, String) {
        let printed = render(format, output, &cx()).unwrap();
        (
            anstream::adapter::strip_str(&printed.out).to_string(),
            anstream::adapter::strip_str(&printed.err).to_string(),
        )
    }

    fn list(value: Value, field: &'static str) -> Fixed {
        Fixed(
            value,
            Shape::List(Listing {
                field,
                ..Listing::default()
            }),
        )
    }

    #[test]
    fn json_is_printed_as_it_always_was() {
        let failure = CommandResult::Failure("no such volume".to_owned());
        assert_eq!(
            printed(OutputFormat::Json, &failure),
            (
                "{\n  \"code\": \"failure\",\n  \"description\": \"no such volume\"\n}\n"
                    .to_owned(),
                String::new()
            )
        );
        let volumes = list(json!({"volumes": [{"name": "v"}]}), "volumes");
        assert_eq!(
            printed(OutputFormat::Json, &volumes).0,
            "{\n  \"volumes\": [\n    {\n      \"name\": \"v\"\n    }\n  ]\n}\n"
        );
        let done = Fixed(
            json!({"code": "success"}),
            Shape::Done("Deleted.".to_owned()),
        );
        assert_eq!(
            printed(OutputFormat::Json, &done).0,
            "{\n  \"code\": \"success\"\n}\n"
        );
    }

    #[test]
    fn a_failure_is_an_error_on_standard_error_in_the_human_formats() {
        let failure = CommandResult::Failure("no such volume\ncode: NOT_FOUND".to_owned());
        for format in [OutputFormat::Table, OutputFormat::Wide, OutputFormat::Name] {
            assert_eq!(
                printed(format, &failure),
                (
                    String::new(),
                    "error: no such volume\ncode: NOT_FOUND\n".to_owned()
                )
            );
        }
        let styled = render(OutputFormat::Table, &failure, &cx()).unwrap().err;
        assert_ne!(styled, anstream::adapter::strip_str(&styled).to_string());
    }

    #[test]
    fn a_command_without_a_result_says_what_it_did() {
        assert_eq!(
            printed(OutputFormat::Table, &CommandResult::Success),
            ("✓ Done.\n".to_owned(), String::new())
        );
        let deleted = Fixed(
            json!({"code": "success"}),
            Shape::Done("Deleted volume v1.".to_owned()),
        );
        assert_eq!(
            printed(OutputFormat::Table, &deleted),
            ("✓ Deleted volume v1.\n".to_owned(), String::new())
        );
        assert_eq!(
            printed(OutputFormat::Name, &CommandResult::Success),
            (String::new(), String::new())
        );
    }

    #[test]
    fn a_list_is_a_table_and_its_next_page_is_said_aside() {
        let page = list(
            json!({"volumes": [{"name": "projects/p/zones/z/volumes/v1", "sizeGib": "10"}],
                   "nextPageToken": "t2"}),
            "volumes",
        );
        assert_eq!(
            printed(OutputFormat::Table, &page),
            (
                "NAME   SIZE GIB\nv1     10\n".to_owned(),
                "More follow: --page-token t2\n".to_owned()
            )
        );
    }

    #[test]
    fn a_list_s_view_and_descriptors_decide_its_columns() {
        let page = Fixed(
            json!({"volumes": [{"name": "projects/p/zones/z/volumes/v1", "sizeGib": 10,
                                "state": "IN_USE", "storageClass": "storageClasses/gp1"}]}),
            Shape::List(Listing {
                field: "volumes",
                item: Some("origincloud.pvs.v1beta1.Volume"),
                columns: &["name", "state", "sizeGib", "storageClass", "attachment"],
                ..Listing::default()
            }),
        );
        assert_eq!(
            printed(OutputFormat::Table, &page).0,
            "NAME   STATE    SIZE GIB   STORAGE CLASS   ATTACHMENT\nv1     IN_USE   10         gp1\n"
        );
    }

    #[test]
    fn an_empty_list_says_where_nothing_was_found_and_how_to_make_one() {
        let empty = list(json!({"instanceTypes": []}), "instanceTypes");
        assert_eq!(
            printed(OutputFormat::Table, &empty),
            (String::new(), "No instance types.\n".to_owned())
        );
        assert_eq!(
            printed(OutputFormat::Name, &empty),
            (String::new(), String::new())
        );

        let listing = |parents: &[&str]| {
            Fixed(
                json!({"instances": []}),
                Shape::List(Listing {
                    field: "instances",
                    parents: parents.iter().map(|p| (*p).to_owned()).collect(),
                    create: Some("origin compute create-instance"),
                    ..Listing::default()
                }),
            )
        };
        assert_eq!(
            printed(OutputFormat::Table, &listing(&["projects/acme/zones/z1"])).1,
            "No instances in project acme, zone z1. Create one with \
             `origin compute create-instance`.\n"
        );
        assert_eq!(
            printed(
                OutputFormat::Table,
                &listing(&["projects/acme/zones/z1", "projects/acme/zones/z2"])
            )
            .1,
            "No instances in any zone of project acme. Create one with \
             `origin compute create-instance`.\n"
        );
    }

    #[test]
    fn a_message_is_its_fields_by_its_descriptor() {
        let volume = Fixed(
            json!({"sizeGib": 10, "name": "projects/p/zones/z/volumes/v1", "state": "AVAILABLE"}),
            Shape::Message("origincloud.pvs.v1beta1.Volume"),
        );
        assert_eq!(
            printed(OutputFormat::Table, &volume).0,
            "name     projects/p/zones/z/volumes/v1\nstate    AVAILABLE\nsizeGib  10\n"
        );
    }

    #[test]
    fn name_prints_each_whole_name() {
        let page = list(
            json!({"volumes": [{"name": "projects/p/zones/z/volumes/v1"},
                               {"name": "projects/p/zones/z/volumes/v2"}]}),
            "volumes",
        );
        assert_eq!(
            printed(OutputFormat::Name, &page).0,
            "projects/p/zones/z/volumes/v1\nprojects/p/zones/z/volumes/v2\n"
        );
        let one = Fixed(json!({"name": "projects/p", "x": 1}), Shape::Inferred);
        assert_eq!(printed(OutputFormat::Name, &one).0, "projects/p\n");
    }

    #[test]
    fn name_refuses_an_output_without_names() {
        let version = Fixed(json!({"version": "1.10.0"}), Shape::Inferred);
        let err = render(OutputFormat::Name, &version, &cx()).unwrap_err();
        assert!(err.to_string().contains("no resource names"), "{err}");
    }

    #[test]
    fn a_graphql_response_is_unwrapped_to_what_it_holds() {
        let roles = Fixed(
            json!({"iam": {"roles": [{"__typename": "IamBuiltinRole", "name": "viewer"}]}}),
            Shape::Inferred,
        );
        assert_eq!(printed(OutputFormat::Table, &roles).0, "NAME\nviewer\n");

        let none = Fixed(json!({"iam": {"roles": []}}), Shape::Inferred);
        assert_eq!(printed(OutputFormat::Table, &none).1, "No roles.\n");

        let user = Fixed(
            json!({"iam": {"currentUser": {"email": "a@b", "userId": "u1"}}}),
            Shape::Inferred,
        );
        assert_eq!(
            printed(OutputFormat::Table, &user).0,
            "email   a@b\nuserId  u1\n"
        );

        let allowed = Fixed(json!({"iam": {"hasPermission": true}}), Shape::Inferred);
        assert_eq!(
            printed(OutputFormat::Table, &allowed).0,
            "hasPermission  true\n"
        );
    }

    #[test]
    fn a_resource_without_descriptors_is_its_fields() {
        let version = Fixed(json!({"version": "1.10.0"}), Shape::Inferred);
        assert_eq!(
            printed(OutputFormat::Table, &version).0,
            "version  1.10.0\n"
        );
    }

    #[test]
    fn raw_bypasses_every_format() {
        struct Raw;
        impl SerializableOutput for Raw {
            fn to_json(&self) -> Result<Value, SerializationError> {
                Ok(json!({"token": "t"}))
            }
            fn to_raw(&self) -> Option<String> {
                Some("t".to_owned())
            }
        }
        for format in [OutputFormat::Table, OutputFormat::Json, OutputFormat::Name] {
            assert_eq!(printed(format, &Raw), ("t\n".to_owned(), String::new()));
        }
    }
}
