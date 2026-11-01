//! Locations the `origin` CLI keeps its configuration in: the environment
//! variables that override the XDG base directories, and the vendor path
//! segment appended to whichever base directory wins.

use std::path::{Path, PathBuf};

pub(super) const ENV_CONFIG_DIR_USER: &str = "XDG_CONFIG_HOME";
pub(super) const ENV_DATA_DIR_USER: &str = "XDG_DATA_HOME";

/// The vendor directory every store reads and writes.
///
/// This is a *filesystem path segment*: changing it does not reinterpret
/// existing data, it points the CLI at a different directory, where it would
/// find no login and no profile.
pub(super) const VENDOR_PORTION: &str = "origincloud";

/// Where a store keeps `file_name` under `base_dir`.
pub(super) fn path(base_dir: &str, file_name: &str) -> PathBuf {
    let path = Path::new(base_dir).join(VENDOR_PORTION).join(file_name);

    expanduser::expanduser(path.to_str().expect("config path is not valid UTF-8"))
        .expect("cannot expand user")
}

/// Serialises the tests that override `HOME`/`XDG_*`.
///
/// `std::env::set_var` mutates state shared by every thread in the test
/// binary, and `cargo test` runs tests in parallel. Any test in this crate
/// that touches the environment must hold this lock for the whole of its
/// setup *and* its assertions.
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned rather than left implicit: a string constant nothing else in
    /// the build would catch a change to, and a live filesystem path holding
    /// someone's login.
    #[test]
    fn the_vendor_segment_is_pinned() {
        assert_eq!(VENDOR_PORTION, "origincloud");
        assert_eq!(
            path("/base", "session.json"),
            PathBuf::from("/base/origincloud/session.json")
        );
    }
}
