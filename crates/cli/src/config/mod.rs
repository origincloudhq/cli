//! On-disk configuration for the `origin` CLI.
//!
//! - `paths.rs`: env-var names, vendor path segment, and the read-new /
//!   fall-back-to-legacy resolution both stores use
//! - `file_io.rs`: the JSON read/write helpers both stores use
//! - `profile.rs`: `ClientProfile`, the user's defaults (username, output
//!   format, project, zone, region)
//! - `session.rs`: `ClientSession`, the cached OIDC tokens
//! - `env.rs`: reading the environment variables that override them

pub(crate) mod env;
mod file_io;
mod paths;
mod profile;
mod session;

pub(crate) use profile::{ClientProfile, OutputFormat};
pub(crate) use session::{Assumed, ClientSession};
pub(crate) use session::{SessionError, EXPIRY_MARGIN};

#[cfg(test)]
pub(crate) use paths::ENV_LOCK;
