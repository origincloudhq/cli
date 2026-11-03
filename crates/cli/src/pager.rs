//! Human output taller than the terminal goes through a pager, as `git`
//! and `gh` page theirs: `ORIGINCLOUD_PAGER`, else `PAGER`, else `less`.
//! Only on a terminal, and only `table` and `wide`: a pipe, a file and
//! `json` get the output as it is.

use std::io::{IsTerminal, Write};
use std::process::{Command, Stdio};

/// The CLI's own pager variable, which comes before `PAGER`.
const ENV_PAGER: &str = "ORIGINCLOUD_PAGER";

/// The pager without either variable.
const DEFAULT: &str = "less";

/// What `less` is given when `LESS` isn't set, as `git` gives it: quit at
/// once when the output fits on the screen (`F`), keep colours (`R`),
/// don't clear the screen on the way out (`X`).
const LESS: &str = "FRX";

/// The pager command for `lines` of output, on a terminal `height` lines
/// tall, as `variable` reads the environment; `None` to print without one:
/// `--no-pager`, a terminal it fits on or whose height isn't known, or a
/// pager variable set to nothing or `cat`.
fn pager(
    lines: usize,
    height: Option<usize>,
    no_pager: bool,
    variable: impl Fn(&str) -> Option<String>,
) -> Option<Vec<String>> {
    if no_pager || lines < height? {
        return None;
    }
    let command = variable(ENV_PAGER)
        .or_else(|| variable("PAGER"))
        .unwrap_or_else(|| DEFAULT.to_owned());
    let words: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
    match words.first().map(String::as_str) {
        None | Some("cat") => None,
        Some(_) => Some(words),
    }
}

/// Prints `out`, styled, on standard output: through the pager when it is
/// a terminal that `out` doesn't fit on, else as it is. A pager that can't
/// be started is gone around; one quit before the end (`q` in `less`)
/// ends the output there.
pub(crate) fn print(out: &str, no_pager: bool) {
    let stdout = std::io::stdout();
    let command = if stdout.is_terminal() {
        pager(
            out.lines().count(),
            crate::output::height(),
            no_pager,
            |v| std::env::var(v).ok(),
        )
    } else {
        None
    };
    let Some(command) = command else {
        anstream::print!("{out}");
        return;
    };

    // Colour as it would be on the terminal, which the pager passes on.
    let choice = anstream::AutoStream::choice(&stdout);
    let mut pager = Command::new(&command[0]);
    pager.args(&command[1..]).stdin(Stdio::piped());
    if std::env::var_os("LESS").is_none() {
        pager.env("LESS", LESS);
    }
    let Ok(mut child) = pager.spawn() else {
        anstream::print!("{out}");
        return;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let text = match choice {
            anstream::ColorChoice::Never => anstream::adapter::strip_str(out).to_string(),
            _ => out.to_owned(),
        };
        // Quitting the pager early closes its input: not a failure.
        let _ = stdin.write_all(text.as_bytes());
    }
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    #[test]
    fn output_taller_than_the_terminal_is_paged_by_less() {
        assert_eq!(
            pager(50, Some(40), false, env(&[])),
            Some(vec!["less".to_owned()])
        );
        // It fits, the height isn't known, or `--no-pager`.
        assert_eq!(pager(39, Some(40), false, env(&[])), None);
        assert_eq!(pager(50, None, false, env(&[])), None);
        assert_eq!(pager(50, Some(40), true, env(&[])), None);
    }

    #[test]
    fn the_cli_s_pager_comes_before_pager_and_nothing_or_cat_turns_it_off() {
        let both = [("ORIGINCLOUD_PAGER", "most -s"), ("PAGER", "more")];
        assert_eq!(
            pager(50, Some(40), false, env(&both)),
            Some(vec!["most".to_owned(), "-s".to_owned()])
        );
        assert_eq!(
            pager(50, Some(40), false, env(&[("PAGER", "more")])),
            Some(vec!["more".to_owned()])
        );
        for off in ["", " ", "cat"] {
            let pairs = [("ORIGINCLOUD_PAGER", off), ("PAGER", "more")];
            assert_eq!(pager(50, Some(40), false, env(&pairs)), None, "{off:?}");
        }
    }
}
