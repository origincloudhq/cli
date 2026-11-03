//! Which format is in effect, whether it is in colour, and how wide the
//! terminal it is laid out for is.

use std::io::IsTerminal;

use crate::config::OutputFormat;

/// The format a command prints in: `chosen`, from `--output`,
/// `ORIGINCLOUD_OUTPUT_FORMAT` or the profile, in that order; without one,
/// `table` when standard output is a terminal, someone reading it, and
/// `json` when it is a pipe or a file, a program reading it, so that a
/// script gets the JSON it always got.
pub(crate) fn format_in_effect(
    chosen: Option<OutputFormat>,
    stdout_is_terminal: bool,
) -> OutputFormat {
    chosen.unwrap_or(if stdout_is_terminal {
        OutputFormat::Table
    } else {
        OutputFormat::Json
    })
}

/// [`format_in_effect`] for this process's standard output.
pub(crate) fn format_for_stdout(chosen: Option<OutputFormat>) -> OutputFormat {
    format_in_effect(chosen, std::io::stdout().is_terminal())
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
    let columns = std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.trim().parse::<usize>().ok())
        .filter(|c| *c > 0);
    columns.or_else(terminal_width)
}

#[cfg(unix)]
fn terminal_width() -> Option<usize> {
    let stdout = std::io::stdout();
    if !stdout.is_terminal() {
        return None;
    }
    rustix::termios::tcgetwinsize(&stdout)
        .ok()
        .map(|size| usize::from(size.ws_col))
        .filter(|c| *c > 0)
}

#[cfg(not(unix))]
fn terminal_width() -> Option<usize> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_choice_a_terminal_gets_a_table_and_a_pipe_json() {
        assert_eq!(format_in_effect(None, true), OutputFormat::Table);
        assert_eq!(format_in_effect(None, false), OutputFormat::Json);
    }

    #[test]
    fn a_choice_holds_on_a_terminal_and_on_a_pipe() {
        for terminal in [true, false] {
            for chosen in [
                OutputFormat::Table,
                OutputFormat::Wide,
                OutputFormat::Json,
                OutputFormat::Name,
            ] {
                assert_eq!(format_in_effect(Some(chosen), terminal), chosen);
            }
        }
    }
}
