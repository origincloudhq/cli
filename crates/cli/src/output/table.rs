//! A list as a table: a column per field, a row per item.

use chrono::{DateTime, Utc};
use prost_reflect::MessageDescriptor;
use serde_json::{Map, Value};

use super::schema::{self, Field, Kind};
use super::style::HEADER;
use super::text;
use super::value::{self, Place, Shown};

/// The space between two columns.
const GAP: &str = "   ";

/// The most columns a cell of `table` takes when the table is fitted to the
/// terminal; `wide` never cuts one.
const MOST_PER_CELL: usize = 40;

/// The fields `table` leaves out unless a view names them, which `wide`
/// shows: what tells one resource from another, and what it is, is
/// elsewhere.
const LEFT_OUT: [&str; 5] = ["urn", "creator", "etag", "uid", "updateTime"];

/// A column: the field it shows, by its JSON path from the item.
#[derive(Debug, Clone)]
pub(super) struct Spec {
    path: Vec<String>,
    field: Field,
}

impl Spec {
    fn new(path: &str, field: Field) -> Self {
        // An item's own name is shown by its id, as references are.
        let field = if path == "name" {
            Field {
                kind: Kind::Reference,
                ..field
            }
        } else {
            field
        };
        Spec {
            path: path.split('.').map(str::to_owned).collect(),
            field,
        }
    }

    fn key(&self) -> String {
        self.path.join(".")
    }
}

/// The columns of a table of `items`:
///
/// - a view's `columns`, the JSON paths `commands.toml` names, first;
/// - in `wide`, or without a view, every other field of `item`, the
///   items' message, that fits in a cell, in the order the API defines
///   them, `name`, `displayName`, `state` and `status` first and
///   `createTime` last; `table` leaves out [`LEFT_OUT`] and maps;
/// - without `item`, the fields the items hold, by name, kinds told from
///   their values, objects left out.
pub(super) fn columns(
    items: &[&Map<String, Value>],
    item: Option<&MessageDescriptor>,
    view: &[&str],
    wide: bool,
) -> Vec<Spec> {
    let mut specs: Vec<Spec> = view
        .iter()
        .map(|path| {
            let steps: Vec<&str> = path.split('.').collect();
            let field = item
                .and_then(|m| schema::path(m, &steps))
                .or_else(|| told(items, &steps))
                .unwrap_or(Field {
                    kind: Kind::Text,
                    repeated: false,
                });
            Spec::new(path, field)
        })
        .collect();
    if !view.is_empty() && !wide {
        return specs;
    }

    let mut rest: Vec<Spec> = match item {
        Some(message) => message
            .fields()
            .map(|f| (f.json_name().to_owned(), Field::of(&f)))
            .filter(|(key, field)| {
                field.fits_in_a_cell()
                    && (wide || (field.kind != Kind::Map && !LEFT_OUT.contains(&key.as_str())))
            })
            .map(|(key, field)| Spec::new(&key, field))
            .collect(),
        None => inferred(items, wide),
    };
    // Stable: the same rank keeps the definitions' order.
    rest.sort_by_key(|s| text::rank(&s.key()));
    for spec in rest {
        if !specs.iter().any(|s| s.key() == spec.key()) {
            specs.push(spec);
        }
    }
    specs
}

/// The kind of what `path` reaches in the first item that has it.
fn told(items: &[&Map<String, Value>], path: &[&str]) -> Option<Field> {
    let path: Vec<String> = path.iter().map(|s| (*s).to_owned()).collect();
    items.iter().find_map(|item| {
        at_item(item, &path)
            .filter(|v| !v.is_null())
            .map(|v| Field::told_from(&v))
    })
}

/// The columns of items without descriptors: every field that holds a
/// scalar, or a list of them, in every item that has it, by name.
fn inferred(items: &[&Map<String, Value>], wide: bool) -> Vec<Spec> {
    let mut kinds: Vec<(String, Field)> = Vec::new();
    let mut nested: Vec<&str> = Vec::new();
    for item in items {
        for (key, value) in *item {
            if key.starts_with("__") || (!wide && LEFT_OUT.contains(&key.as_str())) {
                continue;
            }
            if text::scalar(value).is_none() {
                if !value.is_null() {
                    nested.push(key);
                }
                continue;
            }
            if !kinds.iter().any(|(k, _)| k == key) {
                kinds.push((key.clone(), Field::told_from(value)));
            }
        }
    }
    // A field that holds an object in any item isn't a column in any.
    kinds.retain(|(key, _)| !nested.contains(&key.as_str()));
    kinds.sort_by(|(a, _), (b, _)| a.cmp(b));
    kinds
        .into_iter()
        .map(|(key, field)| Spec::new(&key, field))
        .collect()
}

/// What `path` reaches in `item`.
fn at_item(item: &Map<String, Value>, path: &[String]) -> Option<Value> {
    let (first, rest) = path.split_first()?;
    at(item.get(first)?, rest)
}

/// What `path` reaches in `value`: through a list, the list of what it
/// reaches in each item, flattened.
fn at(value: &Value, path: &[String]) -> Option<Value> {
    let Some((first, rest)) = path.split_first() else {
        return Some(value.clone());
    };
    match value {
        Value::Object(fields) => at(fields.get(first)?, rest),
        Value::Array(items) => {
            let leaves = items
                .iter()
                .filter_map(|item| at(item, path))
                .flat_map(|leaf| match leaf {
                    Value::Array(leaves) => leaves,
                    leaf => vec![leaf],
                })
                .collect();
            Some(Value::Array(leaves))
        }
        _ => None,
    }
}

struct Column {
    header: String,
    cells: Vec<Shown>,
    /// The widest of its header and its cells.
    width: usize,
}

/// A cell: what `spec` reaches in `item`, shown at `place`, or the
/// field's default when the item leaves it out.
fn cell(item: &Map<String, Value>, spec: &Spec, place: Place, now: DateTime<Utc>) -> Shown {
    match at_item(item, &spec.path).filter(|v| !v.is_null()) {
        Some(found) => {
            let shown = value::show(&found, &spec.field, place, now);
            Shown {
                text: shown
                    .as_ref()
                    .map(|s| text::one_line(&s.text))
                    .unwrap_or_default(),
                style: shown.and_then(|s| s.style),
            }
        }
        None => Shown {
            text: spec.field.left_out().to_owned(),
            style: None,
        },
    }
}

/// `items`, objects all, as a table of `specs`, fitted to `width` columns
/// when it is known and `wide` isn't: a cell cut at [`MOST_PER_CELL`], and
/// the columns that don't fit dropped, the first always kept.
pub(super) fn table(
    items: &[&Map<String, Value>],
    specs: &[Spec],
    wide: bool,
    width: Option<usize>,
    now: DateTime<Utc>,
) -> String {
    let place = if wide { Place::Wide } else { Place::Table };
    let mut columns: Vec<Column> = specs
        .iter()
        .map(|spec| {
            let cells: Vec<Shown> = items
                .iter()
                .map(|item| cell(item, spec, place, now))
                .collect();
            let leaf = spec.path.last().map_or("", String::as_str);
            let header = text::one_line(&text::header(leaf));
            let width = cells
                .iter()
                .map(|c| text::width(&c.text))
                .chain([text::width(&header)])
                .max()
                .unwrap_or(0);
            Column {
                header,
                cells,
                width,
            }
        })
        .collect();
    if !wide {
        if let Some(width) = width {
            fit(&mut columns, width);
        }
    }

    let mut out = String::new();
    let header: Vec<(Shown, usize)> = columns
        .iter()
        .map(|c| {
            let shown = Shown {
                text: c.header.clone(),
                style: Some(HEADER),
            };
            (shown, c.width)
        })
        .collect();
    push_row(&mut out, &header);
    for row in 0..items.len() {
        let cells: Vec<(Shown, usize)> = columns
            .iter()
            .map(|c| (c.cells[row].clone(), c.width))
            .collect();
        push_row(&mut out, &cells);
    }
    out
}

/// Fits `columns` to `width`: every cell cut at [`MOST_PER_CELL`], the
/// columns kept in order while they fit, the first always, cut to `width`
/// when it alone is wider.
fn fit(columns: &mut Vec<Column>, width: usize) {
    for column in columns.iter_mut() {
        let most = MOST_PER_CELL.max(text::width(&column.header));
        if column.width > most {
            cut(column, most);
        }
    }

    let mut used = 0;
    let mut kept = 0;
    columns.retain(|column| {
        let needed = if kept == 0 {
            column.width
        } else {
            GAP.len() + column.width
        };
        let keep = kept == 0 || used + needed <= width;
        if keep {
            used += needed;
            kept += 1;
        }
        keep
    });
    if let Some(first) = columns.first_mut() {
        if first.width > width {
            cut(first, width);
        }
    }
}

fn cut(column: &mut Column, width: usize) {
    for cell in column.cells.iter_mut() {
        cell.text = text::truncate(&cell.text, width);
    }
    column.header = text::truncate(&column.header, width);
    column.width = width;
}

/// A row: each cell in its style, padded to its column's width but the
/// last that isn't empty, which would only add spaces at the end of the
/// line.
fn push_row(out: &mut String, cells: &[(Shown, usize)]) {
    let last = cells
        .iter()
        .rposition(|(cell, _)| !cell.text.is_empty())
        .unwrap_or(0);
    for (i, (cell, width)) in cells.iter().enumerate().take(last + 1) {
        if i > 0 {
            out.push_str(GAP);
        }
        match cell.style {
            Some(style) => out.push_str(&format!("{style}{}{style:#}", cell.text)),
            None => out.push_str(&cell.text),
        }
        if i < last {
            out.extend(std::iter::repeat_n(' ', width - text::width(&cell.text)));
        }
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        "2026-11-03T12:00:00Z".parse().unwrap()
    }

    fn message(name: &str) -> MessageDescriptor {
        crate::descriptors::message(name).unwrap()
    }

    fn render(
        items: Value,
        item: Option<&str>,
        view: &[&str],
        wide: bool,
        width: Option<usize>,
    ) -> String {
        let items: Vec<Map<String, Value>> = serde_json::from_value(items).unwrap();
        let items: Vec<&Map<String, Value>> = items.iter().collect();
        let item = item.map(message);
        let specs = columns(&items, item.as_ref(), view, wide);
        anstream::adapter::strip_str(&table(&items, &specs, wide, width, now())).to_string()
    }

    const INSTANCE: &str = "origincloud.compute.v1beta1.Instance";

    fn instances() -> Value {
        json!([
            {"name": "projects/acme/zones/z1/instances/web-1", "state": "RUNNING",
             "instanceType": "instanceTypes/s1.small", "createTime": "2026-10-31T09:12:00Z",
             "interfaceAddresses": [{"networkInterface": "n", "ipv4Address": "10.0.1.4"}],
             "urn": "urn:oc:i:1", "tags": {"env": "prod"}},
            {"name": "projects/acme/zones/z1/instances/batch-7", "state": "STARTING",
             "instanceType": "instanceTypes/c1.xlarge", "createTime": "2026-11-03T11:59:30Z"},
        ])
    }

    #[test]
    fn a_view_names_the_columns_of_table() {
        let printed = render(
            instances(),
            Some(INSTANCE),
            &[
                "name",
                "state",
                "instanceType",
                "interfaceAddresses.ipv4Address",
                "createTime",
            ],
            false,
            None,
        );
        assert_eq!(
            printed,
            "\
NAME      STATE      INSTANCE TYPE   IPV4 ADDRESS   CREATE TIME
web-1     RUNNING    s1.small        10.0.1.4       3d ago
batch-7   STARTING   c1.xlarge                      30s ago
"
        );
    }

    #[test]
    fn without_a_view_the_columns_are_the_definitions_fields_that_fit() {
        let printed = render(instances(), Some(INSTANCE), &[], false, None);
        // The definitions' order, `state` moved up and `createTime` last;
        // the urn, the creator and the tags left out.
        assert_eq!(
            printed.lines().next().unwrap(),
            "NAME      STATE      INSTANCE TYPE   IMAGE   IMAGE FAMILY   KEY PAIR   LOGIN USER   \
             NETWORK INTERFACES   STATE REASON   CREATE TIME"
        );
    }

    #[test]
    fn wide_adds_every_other_field_that_fits_after_the_view() {
        let printed = render(
            instances(),
            Some(INSTANCE),
            &["name", "state"],
            true,
            Some(20),
        );
        let header = printed.lines().next().unwrap();
        assert!(header.starts_with("NAME      STATE      URN"), "{header}");
        assert!(header.contains("TAGS"), "{header}");
        assert!(printed.contains("env=prod"), "{printed}");
    }

    #[test]
    fn a_left_out_field_shows_its_default_by_its_kind() {
        let printed = render(
            json!([
                {"name": "instanceTypes/a", "vcpu": 2, "available": true},
                {"name": "instanceTypes/b"},
            ]),
            Some("origincloud.compute.v1beta1.InstanceType"),
            &["name", "vcpu", "available", "kind"],
            false,
            None,
        );
        assert_eq!(
            printed,
            "NAME   VCPU   AVAILABLE   KIND\na      2      true\nb      0      false\n"
        );
    }

    #[test]
    fn without_descriptors_the_columns_are_told_from_the_items() {
        let printed = render(
            json!([
                {"name": "a", "available": true, "cores": 2, "tags": {"env": "prod"}},
                {"name": "b"},
            ]),
            None,
            &[],
            false,
            None,
        );
        assert_eq!(
            printed,
            "NAME   AVAILABLE   CORES\na      true        2\nb      false       0\n"
        );
    }

    #[test]
    fn a_view_reaches_into_a_nested_object_without_descriptors() {
        let printed = render(
            json!([{"principal": "p1", "user": {"email": "a@b"}, "roleUrns": ["r1", "r2"]}]),
            None,
            &["principal", "user.email", "roleUrns"],
            false,
            None,
        );
        assert_eq!(
            printed,
            "PRINCIPAL   EMAIL   ROLE URNS\np1          a@b     r1, r2\n"
        );
    }

    #[test]
    fn a_table_fits_the_terminal_by_cutting_cells_and_dropping_columns() {
        let long = "k".repeat(60);
        let items = json!([{"name": "web-1", "publicKey": long, "zone": "z1"}]);

        let printed = render(items.clone(), None, &[], false, Some(30));
        assert_eq!(printed, "NAME    ZONE\nweb-1   z1\n");

        let printed = render(items.clone(), None, &[], false, Some(60));
        let cut = format!("{}…", "k".repeat(39));
        assert_eq!(
            printed,
            format!(
                "NAME    PUBLIC KEY{}   ZONE\nweb-1   {cut}   z1\n",
                " ".repeat(30)
            )
        );

        // Not fitted when the width isn't known, as on a pipe.
        let printed = render(items, None, &[], false, None);
        assert!(printed.contains(&"k".repeat(60)), "{printed}");
    }

    #[test]
    fn the_first_column_is_kept_and_cut_when_nothing_else_fits() {
        let printed = render(
            json!([{"name": "a-very-long-name"}]),
            None,
            &[],
            false,
            Some(8),
        );
        assert_eq!(printed, "NAME\na-very-…\n");
    }

    #[test]
    fn a_cell_can_t_carry_terminal_controls() {
        let printed = render(
            json!([{"name": "a", "note": "x\u{1b}[2Jy\nz"}]),
            None,
            &[],
            false,
            None,
        );
        assert_eq!(printed, "NAME   NOTE\na      x\\u{1b}[2Jy z\n");
    }

    #[test]
    fn a_state_is_coloured_and_still_aligned() {
        let items: Vec<Map<String, Value>> = serde_json::from_value(instances()).unwrap();
        let items: Vec<&Map<String, Value>> = items.iter().collect();
        let item = message(INSTANCE);
        let specs = columns(&items, Some(&item), &["state", "name"], false);
        let printed = table(&items, &specs, false, None, now());
        assert!(
            printed.contains("\u{1b}[32mRUNNING\u{1b}[0m"),
            "{printed:?}"
        );
        let plain = anstream::adapter::strip_str(&printed).to_string();
        assert_eq!(
            plain,
            "STATE      NAME\nRUNNING    web-1\nSTARTING   batch-7\n"
        );
    }
}
