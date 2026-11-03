//! The styles of the human formats. They are written into the text
//! whatever the terminal; `anstream` removes them on the way out where
//! colour isn't wanted (see [`super::ColorWhen`]).

use anstyle::{AnsiColor, Style};

/// A table's header row.
pub(super) const HEADER: Style = Style::new().bold();

/// The `error:` before a failure's description.
pub(super) const ERROR: Style = AnsiColor::Red.on_default().bold();
