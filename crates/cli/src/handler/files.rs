//! Writing the files of the coding agents, shared by `origin mcp install`
//! and `origin skill install`: a path is written at the target of its
//! symbolic links, so that a link, such as one into a dotfiles repository,
//! stays one, and a new file has the permissions of the one it replaces
//! from its creation on.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};

/// The file `path` names after following its symbolic links, which may
/// be relative to the directory of the link, or dangle.
pub(crate) fn resolve_links(path: &Path) -> anyhow::Result<PathBuf> {
    let mut path = path.to_path_buf();
    // As many as the kernel follows before it gives up (ELOOP).
    for _ in 0..40 {
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = std::fs::read_link(&path)
                    .with_context(|| format!("reading the link {}", path.display()))?;
                path = match path.parent() {
                    Some(dir) if target.is_relative() => dir.join(target),
                    _ => target,
                };
            }
            // Its directory without the `..` a relative link brings, for
            // the paths the output names.
            _ => {
                return Ok(match (path.parent(), path.file_name()) {
                    (Some(dir), Some(name)) => match std::fs::canonicalize(dir) {
                        Ok(dir) => dir.join(name),
                        Err(_) => path,
                    },
                    _ => path,
                })
            }
        }
    }
    bail!("{}: too many levels of symbolic links", path.display())
}

/// Writes `content` to the new file `path`, replacing one left behind by
/// an earlier run; with `permissions`, the file has them from its creation
/// on, never more open than they are.
pub(crate) fn create(
    path: &Path,
    content: impl AsRef<[u8]>,
    permissions: Option<&std::fs::Permissions>,
) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if let Some(permissions) = permissions {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        // The umask can only narrow this.
        options.mode(permissions.mode() & 0o7777);
    }
    let mut out = options.open(path)?;
    out.write_all(content.as_ref())?;
    out.sync_all()?;
    drop(out);
    // Exactly the file's, which the umask may have narrowed.
    if let Some(permissions) = permissions {
        std::fs::set_permissions(path, permissions.clone())?;
    }
    Ok(())
}

pub(crate) fn sibling(file: &Path, suffix: &str) -> PathBuf {
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    file.with_file_name(name)
}
