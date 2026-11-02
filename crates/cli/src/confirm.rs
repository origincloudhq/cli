//! The confirmation guard for destructive commands.
//!
//! Every irreversible command (every delete, `revoke-service-account`, and
//! the changes the generator's table names, such as growing a volume)
//! requires an explicit `--yes`.
//!
//! The guard is a type, not a convention: the client method behind a
//! destructive command takes a [`Confirmed`], whose field is private and
//! whose only constructor is [`require`]. A call that skips the check does
//! not compile, and the refusal happens before any request is built.

use clap::Args;

/// Proof that a destructive operation was explicitly acknowledged.
///
/// Constructible only by [`require`]: the unit field is private, so no other
/// module can make one.
#[must_use]
pub struct Confirmed(());

/// What the command is about to do, in the words the refusal uses.
pub struct Destructive<'a> {
    /// The verb, such as "delete" or "terminate".
    pub verb: &'a str,
    /// The kind of thing, such as "volume".
    pub what: &'a str,
    /// Which one, such as its name or URN.
    pub target: &'a str,
    /// What is irreversibly lost, for someone deciding whether to go on.
    pub consequence: &'a str,
}

/// The `--yes` flag, flattened into every destructive command so that its
/// spelling and help have one source.
///
/// A bare flag is right here: it sets nothing on the resource, it authorises
/// one invocation.
#[derive(Args, Debug, Clone, Copy)]
pub struct Confirmation {
    /// Confirm an irreversible operation: a delete, or a change the help
    /// says can't be undone. Without it the command refuses, and says what it
    /// would have done, before sending anything.
    #[arg(long)]
    pub yes: bool,
}

/// Proof of confirmation, or an error saying what would have happened and
/// how to proceed.
pub fn require(confirmation: Confirmation, op: Destructive<'_>) -> anyhow::Result<Confirmed> {
    if confirmation.yes {
        return Ok(Confirmed(()));
    }
    anyhow::bail!("{}", refusal(&op, &reinvocation(std::env::args())))
}

/// The refusal text, pure so that its wording is pinned by tests.
fn refusal(op: &Destructive<'_>, reinvocation: &str) -> String {
    format!(
        "refusing to {} {} {} without confirmation. {} To proceed, re-run: {reinvocation}",
        op.verb, op.what, op.target, op.consequence
    )
}

/// The command line as it was given, with `--yes` added, quoted for a
/// POSIX shell where it has to be.
fn reinvocation(args: impl IntoIterator<Item = String>) -> String {
    let mut words: Vec<String> = args.into_iter().map(|a| quote(&a)).collect();
    if let Some(program) = words.first_mut() {
        // The program as people type it, not the path it was started by.
        if let Some(name) = std::path::Path::new(program.as_str())
            .file_name()
            .and_then(|n| n.to_str())
        {
            *program = name.to_owned();
        }
    }
    words.push("--yes".to_owned());
    words.join(" ")
}

fn quote(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./:=@,+%".contains(&b));
    if plain {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Destructive<'static> {
        Destructive {
            verb: "delete",
            what: "volume",
            target: "projects/p/zones/z/volumes/v",
            consequence: "Its data can't be recovered.",
        }
    }

    #[test]
    fn confirmation_is_granted_only_with_the_flag() {
        assert!(require(Confirmation { yes: true }, sample()).is_ok());
        assert!(require(Confirmation { yes: false }, sample()).is_err());
    }

    /// The refusal answers what is about to be lost and what to type now.
    #[test]
    fn the_refusal_names_the_target_the_loss_and_the_reinvocation() {
        let message = refusal(&sample(), "origin pvs delete-volume --volume v --yes");
        assert!(
            message.contains("projects/p/zones/z/volumes/v"),
            "{message}"
        );
        assert!(message.contains("recovered"), "{message}");
        assert!(message.ends_with("--yes"), "{message}");
    }

    #[test]
    fn the_reinvocation_is_the_command_line_with_yes() {
        let args = [
            "/usr/bin/origin",
            "pvs",
            "delete-volume",
            "--volume",
            "my vol",
        ]
        .map(String::from);
        assert_eq!(
            reinvocation(args),
            "origin pvs delete-volume --volume 'my vol' --yes"
        );
    }
}
