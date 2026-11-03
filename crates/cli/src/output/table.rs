//! A list as a table: a column per field, a row per item.

use serde_json::{Map, Value};

use super::style::HEADER;
use super::text;

/// The space between two columns.
const GAP: &str = "   ";

/// The most columns a cell of `table` takes when the table is fitted to the
/// terminal; `wide` never cuts one.
const MOST_PER_CELL: usize = 40;

/// The fields the `table` format leaves out, which `wide` shows: what
/// tells one resource from another, or what it is, is elsewhere.
const LEFT_OUT: [&str; 3] = ["etag", "uid", "updateTime"];

/// What a field of the items holds, which decides what a cell shows for an
/// item that leaves the field out. The API's JSON leaves out a field at its
/// default, so that an item without `available` is one that isn't: its
/// cell says `false`, not nothing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bool,
    Number,
    Text,
}

impl Kind {
    fn of(value: &Value) -> Self {
        match value {
            Value::Bool(_) => Kind::Bool,
            Value::Number(_) => Kind::Number,
            _ => Kind::Text,
        }
    }

    /// What a cell shows for an item without the field.
    fn left_out(self) -> &'static str {
        match self {
            Kind::Bool => "false",
            Kind::Number => "0",
            Kind::Text => "",
        }
    }
}

struct Column {
    header: String,
    cells: Vec<String>,
    /// The widest of its header and its cells.
    width: usize,
}

/// `items`, objects all, as a table: in `wide`, every field that fits in a
/// cell (a scalar, or a list of them); otherwise those but [`LEFT_OUT`],
/// fitted to `width` columns when it is known, a cell cut at
/// [`MOST_PER_CELL`] and the columns that don't fit dropped, the first
/// always kept. A field holding an object is never a column: the detail
/// of a resource, or `json`, shows it.
pub(super) fn table(items: &[&Map<String, Value>], wide: bool, width: Option<usize>) -> String {
    let mut columns = columns(items, wide);
    if !wide {
        if let Some(width) = width {
            fit(&mut columns, width);
        }
    }

    let mut out = String::new();
    let header: Vec<(&str, usize)> = columns
        .iter()
        .map(|c| (c.header.as_str(), c.width))
        .collect();
    push_row(&mut out, &header, Some(HEADER));
    for row in 0..items.len() {
        let cells: Vec<(&str, usize)> = columns
            .iter()
            .map(|c| (c.cells[row].as_str(), c.width))
            .collect();
        push_row(&mut out, &cells, None);
    }
    out
}

/// The table's columns, in order: `name`, `state`, `status`, the other
/// fields by name, and `createTime` last.
fn columns(items: &[&Map<String, Value>], wide: bool) -> Vec<Column> {
    let mut kinds: Vec<(String, Kind)> = Vec::new();
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
                kinds.push((key.clone(), Kind::of(value)));
            }
        }
    }
    // A field that holds an object in any item isn't a column in any.
    kinds.retain(|(key, _)| !nested.contains(&key.as_str()));
    kinds.sort_by(|(a, _), (b, _)| text::rank(a).cmp(&text::rank(b)).then_with(|| a.cmp(b)));

    kinds
        .into_iter()
        .map(|(key, kind)| {
            let cells: Vec<String> = items
                .iter()
                .map(|item| match item.get(&key).and_then(text::scalar) {
                    Some(value) if key == "name" => text::one_line(text::short_name(&value)),
                    Some(value) => text::one_line(&value),
                    None => kind.left_out().to_owned(),
                })
                .collect();
            let header = text::one_line(&text::header(&key));
            let width = cells
                .iter()
                .map(|c| text::width(c))
                .chain([text::width(&header)])
                .max()
                .unwrap_or(0);
            Column {
                header,
                cells,
                width,
            }
        })
        .collect()
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
        *cell = text::truncate(cell, width);
    }
    column.header = text::truncate(&column.header, width);
    column.width = width;
}

/// A row: each cell padded to its column's width but the last, which
/// would only add spaces at the end of the line.
fn push_row(out: &mut String, cells: &[(&str, usize)], style: Option<anstyle::Style>) {
    let mut line = String::new();
    for (i, (cell, width)) in cells.iter().enumerate() {
        if i > 0 {
            line.push_str(GAP);
        }
        line.push_str(cell);
        if i + 1 < cells.len() {
            line.extend(std::iter::repeat_n(' ', width - text::width(cell)));
        }
    }
    let line = line.trim_end();
    match style {
        Some(style) => out.push_str(&format!("{style}{line}{style:#}\n")),
        None => {
            out.push_str(line);
            out.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn render(items: Value, wide: bool, width: Option<usize>) -> String {
        let items: Vec<Map<String, Value>> = serde_json::from_value(items).unwrap();
        let items: Vec<&Map<String, Value>> = items.iter().collect();
        anstream::adapter::strip_str(&table(&items, wide, width)).to_string()
    }

    #[test]
    fn a_list_is_a_column_per_field_its_name_first_and_short() {
        let printed = render(
            json!([
                {"name": "projects/acme/zones/z1/instances/web-1", "state": "RUNNING",
                 "instanceType": "s1.small", "createTime": "2026-10-31T09:12:00Z"},
                {"name": "projects/acme/zones/z1/instances/batch-7", "state": "STARTING",
                 "instanceType": "c1.xlarge", "createTime": "2026-11-03T10:00:00Z"},
            ]),
            false,
            None,
        );
        assert_eq!(
            printed,
            "\
NAME      STATE      INSTANCE TYPE   CREATE TIME
web-1     RUNNING    s1.small        2026-10-31T09:12:00Z
batch-7   STARTING   c1.xlarge       2026-11-03T10:00:00Z
"
        );
    }

    #[test]
    fn a_field_left_out_of_an_item_shows_its_default() {
        let printed = render(
            json!([
                {"name": "a", "available": true, "cores": 2, "family": "x"},
                {"name": "b"},
            ]),
            false,
            None,
        );
        assert_eq!(
            printed,
            "\
NAME   AVAILABLE   CORES   FAMILY
a      true        2       x
b      false       0
"
        );
    }

    #[test]
    fn objects_are_not_columns_and_lists_of_scalars_are() {
        let printed = render(
            json!([
                {"name": "a", "tags": {"env": "prod"}, "zones": ["z1", "z2"]},
                {"name": "b", "zones": ["z1"]},
            ]),
            false,
            None,
        );
        assert_eq!(printed, "NAME   ZONES\na      z1, z2\nb      z1\n");
    }

    #[test]
    fn table_leaves_out_what_wide_shows() {
        let items = json!([{"name": "a", "etag": "e1", "uid": "u1", "updateTime": "t"}]);
        assert_eq!(render(items.clone(), false, None), "NAME\na\n");
        assert_eq!(
            render(items, true, Some(10)),
            "NAME   ETAG   UID   UPDATE TIME\na      e1     u1    t\n"
        );
    }

    #[test]
    fn a_table_fits_the_terminal_by_cutting_cells_and_dropping_columns() {
        let long = "k".repeat(60);
        let items = json!([{"name": "web-1", "publicKey": long, "zone": "z1"}]);

        let printed = render(items.clone(), false, Some(30));
        assert_eq!(printed, "NAME    ZONE\nweb-1   z1\n");

        let printed = render(items.clone(), false, Some(60));
        let cut = format!("{}…", "k".repeat(39));
        assert_eq!(
            printed,
            format!(
                "NAME    PUBLIC KEY{}   ZONE\nweb-1   {cut}   z1\n",
                " ".repeat(30)
            )
        );

        // Not fitted when the width isn't known, as on a pipe.
        let printed = render(items, false, None);
        assert!(printed.contains(&"k".repeat(60)), "{printed}");
    }

    #[test]
    fn the_first_column_is_kept_and_cut_when_nothing_else_fits() {
        let printed = render(json!([{"name": "a-very-long-name"}]), false, Some(8));
        assert_eq!(printed, "NAME\na-very-…\n");
    }

    #[test]
    fn a_cell_can_t_carry_terminal_controls() {
        let printed = render(
            json!([{"name": "a", "note": "x\u{1b}[2Jy\nz"}]),
            false,
            None,
        );
        assert_eq!(printed, "NAME   NOTE\na      x\\u{1b}[2Jy z\n");
    }
}
