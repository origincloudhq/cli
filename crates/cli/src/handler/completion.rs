//! `origin completion`: the script completing `origin` in a shell, from
//! the command tree itself, so that it knows every command and flag the
//! CLI has, the generated ones included, and the values of those that take
//! one of a few.

use std::io::Write;

use clap_complete::Shell;

/// Writes `shell`'s completion script for `origin` to `out`.
pub(crate) fn write(shell: Shell, out: &mut impl Write) {
    let mut command = crate::command();
    clap_complete::generate(shell, &mut command, "origin", out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shell_s_script_completes_the_generated_groups_too() {
        for shell in [
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::PowerShell,
            Shell::Elvish,
        ] {
            let mut script = Vec::new();
            write(shell, &mut script);
            let script = String::from_utf8(script).unwrap();
            // Fish writes a long flag `-l output`, the others `--output`.
            for word in ["origin", "list-volumes", "completion", "output"] {
                assert!(script.contains(word), "{shell}: {word}");
            }
        }
    }
}
