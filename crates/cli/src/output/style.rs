//! The styles of the human formats. They are written into the text
//! whatever the terminal; `anstream` removes them on the way out where
//! colour isn't wanted (see [`super::ColorWhen`]).

use anstyle::{AnsiColor, Style};

/// A table's header row.
pub(super) const HEADER: Style = Style::new().bold();

/// The `error:` before a failure's description.
pub(super) const ERROR: Style = AnsiColor::Red.on_default().bold();

/// The `✓` before what a command without a result did.
pub(super) const DONE: Style = AnsiColor::Green.on_default().bold();

/// A state of something up and in use: `RUNNING`, `AVAILABLE`.
pub(super) const GOOD: Style = AnsiColor::Green.on_default();

/// A state on its way to another: `STARTING`, `PENDING`.
pub(super) const CHANGING: Style = AnsiColor::Yellow.on_default();

/// A state of something that failed: `FAILED`, `ERROR`.
pub(super) const BAD: Style = AnsiColor::Red.on_default().bold();

/// A state of something stopped or gone: `STOPPED`, `DELETED`.
pub(super) const GONE: Style = Style::new().dimmed();
