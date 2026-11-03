//! What a command says on standard error while it waits for an operation:
//! a spinner, what it waits for and how long it has, on one line that is
//! cleared once the wait is over. Only on a terminal: a pipe or a file
//! gets nothing, as it always has.

use std::io::{IsTerminal, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

/// The spinner's frames, one each [`TICK`].
const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

const TICK: Duration = Duration::from_millis(100);

/// Back to the line's start, and the line erased.
const CLEAR: &str = "\r\x1b[2K";

/// The exit status of a command stopped with Ctrl-C, as a shell gives one
/// killed by `SIGINT`.
const INTERRUPTED: i32 = 130;

/// A spinner on standard error, there until it is dropped.
pub(crate) struct Spinner {
    /// Whether it has stopped, under the lock its lines are written with,
    /// so that no frame is drawn after the line is cleared.
    stopped: Arc<Mutex<bool>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Spinner {
    /// Starts a spinner saying `label` (`Creating volume v1`), and how long
    /// it has spun, when standard error is a terminal; otherwise nothing.
    ///
    /// Ctrl-C while it spins stops waiting, not the operation: the line is
    /// cleared, `resume` says how to wait again, and the command exits with
    /// status 130.
    pub(crate) fn start(label: String, resume: String) -> Self {
        let stopped = Arc::new(Mutex::new(false));
        let shows = std::io::stderr().is_terminal()
            && std::env::var("TERM").map_or(true, |term| term != "dumb");
        if !shows {
            return Spinner {
                stopped,
                task: None,
            };
        }
        let lock = Arc::clone(&stopped);
        let task = tokio::spawn(async move {
            let started = Instant::now();
            let mut tick = tokio::time::interval(TICK);
            let interrupted = tokio::signal::ctrl_c();
            tokio::pin!(interrupted);
            let mut frame = 0;
            loop {
                tokio::select! {
                    _ = tick.tick() => {
                        let stopped = lock.lock().unwrap_or_else(|e| e.into_inner());
                        if *stopped {
                            return;
                        }
                        let line = format!(
                            "{CLEAR}{} {label}… {}",
                            FRAMES[frame % FRAMES.len()],
                            elapsed(started.elapsed())
                        );
                        let mut err = std::io::stderr().lock();
                        let _ = err.write_all(line.as_bytes());
                        let _ = err.flush();
                        frame += 1;
                    }
                    _ = &mut interrupted => {
                        let _stopped = lock.lock().unwrap_or_else(|e| e.into_inner());
                        eprint!("{CLEAR}");
                        eprintln!("{resume}");
                        std::process::exit(INTERRUPTED);
                    }
                }
            }
        });
        Spinner {
            stopped,
            task: Some(task),
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        let Some(task) = self.task.take() else {
            return;
        };
        let mut stopped = self.stopped.lock().unwrap_or_else(|e| e.into_inner());
        *stopped = true;
        let mut err = std::io::stderr().lock();
        let _ = err.write_all(CLEAR.as_bytes());
        let _ = err.flush();
        task.abort();
    }
}

/// How long a wait has lasted: `14s`, `2m05s`, `1h02m`.
pub(crate) fn elapsed(span: Duration) -> String {
    let seconds = span.as_secs();
    match seconds {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m{:02}s", s / 60, s % 60),
        s => format!("{}h{:02}m", s / 3600, (s % 3600) / 60),
    }
}

/// What a spinner says it waits for, from the operation's JSON: its
/// metadata's verb and target, `Creating volume v1` for `create` and
/// `projects/p/zones/z/volumes/v1`; without them, `Waiting for` the
/// operation.
pub(crate) fn label(operation: &Value) -> String {
    let metadata = operation.get("metadata");
    let field = |key: &str| {
        metadata
            .and_then(|m| m.get(key))
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
    };
    let name = operation
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (Some(verb), Some(target)) = (field("verb"), field("target")) else {
        return format!("Waiting for {name}");
    };
    let segments: Vec<&str> = target.split('/').collect();
    let what = match segments.as_slice() {
        [.., collection, id] => format!("{} {id}", crate::output::singular(collection)),
        _ => target.to_owned(),
    };
    format!("{} {what}", gerund(verb))
}

/// `verb`, capitalised, as what is going on: `Creating` for `create`,
/// `Stopping` for `stop`, `Attaching` for `attach`.
fn gerund(verb: &str) -> String {
    let verb = verb.trim().to_lowercase();
    let stem = match verb.as_str() {
        "stop" => "stopp".to_owned(),
        v if v.ends_with('e') && !v.ends_with("ee") => v[..v.len() - 1].to_owned(),
        v => v.to_owned(),
    };
    let mut gerund = format!("{stem}ing");
    if let Some(first) = gerund.get(..1) {
        gerund = format!("{}{}", first.to_uppercase(), &gerund[1..]);
    }
    gerund
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_wait_says_what_goes_on_from_the_operation_s_metadata() {
        let operation = |verb: &str, target: &str| {
            json!({"name": "projects/p/zones/z/operations/op-1",
                   "metadata": {"verb": verb, "target": target}})
        };
        assert_eq!(
            label(&operation("create", "projects/p/zones/z/volumes/v1")),
            "Creating volume v1"
        );
        assert_eq!(
            label(&operation("stop", "projects/p/zones/z/instances/web-1")),
            "Stopping instance web-1"
        );
        assert_eq!(
            label(&operation(
                "attach",
                "projects/p/zones/z/instances/i/volumeAttachments/va"
            )),
            "Attaching volume attachment va"
        );
        assert_eq!(
            label(&operation("delete", "projects/p/regions/r/dnsPolicies/d")),
            "Deleting dns policy d"
        );
        assert_eq!(
            label(&json!({"name": "projects/p/zones/z/operations/op-1"})),
            "Waiting for projects/p/zones/z/operations/op-1"
        );
    }

    #[test]
    fn how_long_a_wait_has_lasted() {
        assert_eq!(elapsed(Duration::from_secs(14)), "14s");
        assert_eq!(elapsed(Duration::from_secs(125)), "2m05s");
        assert_eq!(elapsed(Duration::from_secs(3720)), "1h02m");
    }
}
