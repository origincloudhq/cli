//! `origin completion`: the script completing `origin` in a shell, from
//! the command tree itself, so that it knows every command and flag the
//! CLI has, the generated ones included, and the values of those that take
//! one of a few.
//!
//! The script is for a file or a pipe (`> file`, `eval "$(…)"`), where it is
//! printed. On a terminal, where it would scroll past unused, the command
//! says how to install it instead.

use std::io::{IsTerminal, Write};

use clap_complete::Shell;

/// What `origin completion` prints, for `shell`, or the one `SHELL` names,
/// and its exit status: on a terminal, how to install the script; on a
/// pipe or in a file, the script.
pub(crate) fn run(shell: Option<Shell>) -> i32 {
    let Some(shell) = shell.or_else(Shell::from_env) else {
        eprintln!(
            "error: SHELL doesn't name a shell `origin` completes in: name it, as in `origin \
             completion bash` (bash, zsh, fish, powershell or elvish)"
        );
        return 2;
    };
    let mut stdout = std::io::stdout();
    if stdout.is_terminal() {
        print!("{}", instructions(shell));
        return 0;
    }
    // Written whole, so that a reader closing the pipe early (`| head`)
    // ends it there rather than failing the command.
    let mut script = Vec::new();
    write(shell, &mut script);
    let _ = stdout.write_all(&script);
    0
}

/// Writes `shell`'s completion script for `origin` to `out`.
pub(crate) fn write(shell: Shell, out: &mut impl Write) {
    let mut command = crate::command();
    clap_complete::generate(shell, &mut command, "origin", out);
}

/// How to install `shell`'s script, in commands to paste.
pub(crate) fn instructions(shell: Shell) -> String {
    let steps = match shell {
        Shell::Bash => {
            "\
Run these once, then open a new shell:

  mkdir -p ~/.local/share/bash-completion/completions
  origin completion bash > ~/.local/share/bash-completion/completions/origin

That directory is read by the bash-completion package (`apt install
bash-completion`, or `brew install bash-completion@2` on macOS). Without
it, add this line to ~/.bashrc instead:

  eval \"$(origin completion bash)\"
"
        }
        Shell::Zsh => {
            "\
Add this line to ~/.zshrc, after `compinit` (which oh-my-zsh and most
setups run already; otherwise put `autoload -Uz compinit && compinit`
before it), then open a new shell:

  eval \"$(origin completion zsh)\"
"
        }
        Shell::Fish => {
            "\
Run these once, then open a new shell:

  mkdir -p ~/.config/fish/completions
  origin completion fish > ~/.config/fish/completions/origin.fish
"
        }
        Shell::PowerShell => {
            "\
Run these once, then open a new PowerShell:

  if (!(Test-Path $PROFILE)) { New-Item -ItemType File -Force $PROFILE }
  Add-Content $PROFILE 'origin completion powershell | Out-String | Invoke-Expression'
"
        }
        Shell::Elvish => {
            "\
Run these once, then open a new shell:

  mkdir -p ~/.config/elvish
  echo 'eval (origin completion elvish | slurp)' >> ~/.config/elvish/rc.elv
"
        }
        _ => "Redirect the script into a file your shell loads at start.\n",
    };
    format!(
        "To complete `origin`'s commands, flags and values in {shell}:\n\n{steps}\n\
         (This is how to install it, as the output is a terminal. Into a file or a\n\
         pipe, `origin completion {shell}` prints the script itself.)\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHELLS: [Shell; 5] = [
        Shell::Bash,
        Shell::Zsh,
        Shell::Fish,
        Shell::PowerShell,
        Shell::Elvish,
    ];

    #[test]
    fn every_shell_s_script_completes_the_generated_groups_too() {
        for shell in SHELLS {
            let mut script = Vec::new();
            write(shell, &mut script);
            let script = String::from_utf8(script).unwrap();
            // Fish writes a long flag `-l output`, the others `--output`.
            for word in ["origin", "list-volumes", "completion", "output"] {
                assert!(script.contains(word), "{shell}: {word}");
            }
        }
    }

    #[test]
    fn every_shell_has_its_own_steps_that_install_its_script() {
        for shell in SHELLS {
            let text = instructions(shell);
            assert!(
                text.contains(&format!("origin completion {shell}")),
                "{shell}: {text}"
            );
            assert!(!text.contains("Redirect the script"), "{shell}: {text}");
        }
    }
}
