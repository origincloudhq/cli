//! JSON read/write helpers shared by the profile and the session store.
//!
//! A write never leaves a partial file behind: the content goes to a
//! temporary file beside the target, readable by its owner only, which is
//! flushed to disk and then renamed over the target. A reader, in this
//! process or another, sees the old file or the new one, never a mix.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

pub(super) fn read_file_opt<T>(path: &Path) -> Option<T>
where
    T: DeserializeOwned,
{
    let fs = File::open(path).ok()?;
    serde_json::from_reader(fs).ok()
}

/// Creates `dir` and its missing parents, each readable by its owner only.
pub(super) fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

/// Opens `path` for writing as a new file readable by its owner only.
pub(super) fn create_private_file(path: &Path, create_new: bool) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).read(true);
    if create_new {
        options.create_new(true);
    } else {
        options.create(true).truncate(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Writes `content` as JSON to `path`, atomically.
pub(super) fn safe_file<T>(path: &Path, content: &T) -> Result<(), std::io::Error>
where
    T: Serialize,
{
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    create_private_dir(dir)?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("config");
    let data = serde_json::to_vec_pretty(content).map_err(std::io::Error::other)?;

    let temporary = dir.join(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let written = (|| {
        let mut file = create_private_file(&temporary, true)?;
        file.write_all(&data)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        // The rename itself is on disk once the directory is.
        if let Ok(directory) = File::open(dir) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[cfg(unix)]
    #[test]
    fn a_written_file_and_its_new_directory_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new").join("session.json");

        safe_file(&path, &serde_json::json!({"a": 1})).unwrap();

        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
        // Rewriting an existing file keeps it private, and leaves no
        // temporary file behind.
        safe_file(&path, &serde_json::json!({"a": 2})).unwrap();
        assert_eq!(mode(&path), 0o600);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }

    /// Two writers racing, and a reader beside them: every read parses, as
    /// one writer's content or the other's.
    #[test]
    fn concurrent_writes_never_leave_an_unparseable_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = Arc::new(dir.path().join("session.json"));
        let big = |n: u32| serde_json::json!({ "writer": n, "padding": "x".repeat(64 * 1024) });
        safe_file(&path, &big(0)).unwrap();

        let writers: Vec<_> = (1..=2)
            .map(|n| {
                let path = Arc::clone(&path);
                std::thread::spawn(move || {
                    for _ in 0..200 {
                        safe_file(&path, &big(n)).unwrap();
                    }
                })
            })
            .collect();
        let mut reads = 0;
        while writers.iter().any(|w| !w.is_finished()) {
            let value: serde_json::Value =
                read_file_opt(&path).expect("every read parses while writers race");
            assert!(value["writer"].as_u64().is_some());
            reads += 1;
        }
        for w in writers {
            w.join().unwrap();
        }
        assert!(reads > 0);
        let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(left.len(), 1, "no temporary file is left behind");
    }
}
