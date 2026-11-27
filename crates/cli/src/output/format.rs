//! Which format is in effect, whether it is in colour, and how wide the
//! terminal it is laid out for is.

use std::io::IsTerminal;

use crate::config::OutputFormat;

/// The format a command prints in: `chosen`, from `--output`,
/// `ORIGINCLOUD_OUTPUT_FORMAT` or the profile, in that order; without one,
/// `table`, whether standard output is a terminal, a pipe or a file, so
/// that a command prints the same wherever its output goes. A script asks
/// for JSON with `-o json`, or `ORIGINCLOUD_OUTPUT_FORMAT=json`.
pub(crate) fn format_for_stdout(chosen: Option<OutputFormat>) -> OutputFormat {
    chosen.unwrap_or(OutputFormat::Table)
}

/// When the human formats use colour, from `--color`. `json` never does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ColorWhen {
    /// On a terminal, unless `NO_COLOR` is set (`CLICOLOR_FORCE` forces it).
    Auto,
    /// Always, through a pipe too.
    Always,
    /// Never.
    Never,
}

impl ColorWhen {
    /// Makes this the choice of everything printed through `anstream`.
    pub(crate) fn apply(self) {
        let choice = match self {
            ColorWhen::Auto => anstream::ColorChoice::Auto,
            ColorWhen::Always => anstream::ColorChoice::Always,
            ColorWhen::Never => anstream::ColorChoice::Never,
        };
        choice.write_global();
    }
}

/// How many columns a table is fitted to: `COLUMNS` when it holds a
/// number, as for `ls`; else the terminal's width when standard output is
/// one; else `None`, and a table isn't cut to fit, as on a pipe.
pub(crate) fn width() -> Option<usize> {
    number("COLUMNS").or_else(|| window().map(|(columns, _)| columns))
}

/// How many lines the terminal shows, which output longer than that is
/// paged for: `LINES` when it holds a number, else the terminal's height
/// when standard output is one; else `None`.
pub(crate) fn height() -> Option<usize> {
    number("LINES").or_else(|| window().map(|(_, lines)| lines))
}

fn number(variable: &str) -> Option<usize> {
    std::env::var(variable)
        .ok()
        .and_then(|c| c.trim().parse::<usize>().ok())
        .filter(|c| *c > 0)
}

/// The terminal's columns and lines, when standard output is one.
#[cfg(unix)]
fn window() -> Option<(usize, usize)> {
    let stdout = std::io::stdout();
    if !stdout.is_terminal() {
        return None;
    }
    rustix::termios::tcgetwinsize(&stdout)
        .ok()
        .map(|size| (usize::from(size.ws_col), usize::from(size.ws_row)))
        .filter(|(columns, lines)| *columns > 0 && *lines > 0)
}

#[cfg(not(unix))]
fn window() -> Option<(usize, usize)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_choice_the_format_is_a_table() {
        assert_eq!(format_for_stdout(None), OutputFormat::Table);
    }

    #[test]
    fn a_choice_holds() {
        for chosen in [
            OutputFormat::Table,
            OutputFormat::Wide,
            OutputFormat::Json,
            OutputFormat::Name,
        ] {
            assert_eq!(format_for_stdout(Some(chosen)), chosen);
        }
    }
}
