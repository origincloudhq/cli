//! Reading the environment variables that override the saved profile.

use std::env;
use std::str::FromStr;

/// The variable `key`, or `default` when it is unset, empty or doesn't parse.
pub(crate) fn env_or<T>(key: &str, default: T) -> T
where
    T: FromStr,
{
    try_env(key).unwrap_or(default)
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("parsing failed")]
    Parse,

    #[error("set but empty (or whitespace-only)")]
    EmptyValue,

    #[error(transparent)]
    Var(#[from] env::VarError),
}

/// Reads and parses the environment variable `key`.
///
/// A variable that is set but empty, or holds only whitespace, is refused as
/// [`Error::EmptyValue`] rather than parsed to `""`: "set to nothing" means
/// the same as "absent". Other values are parsed unmodified (no trimming).
pub(crate) fn try_env<T>(key: &str) -> Result<T, Error>
where
    T: FromStr,
{
    let value = env::var(key)?;
    if value.trim().is_empty() {
        return Err(Error::EmptyValue);
    }
    value.parse().map_err(|_| Error::Parse)
}

/// Reads a variable the CLI can't run without, such as `HOME`, and panics
/// naming it and the reason when it is unset, empty or doesn't parse.
pub(crate) fn expect_env<T>(key: &str) -> T
where
    T: FromStr,
{
    match try_env(key) {
        Ok(value) => value,
        Err(e) => panic!("Mandatory environment variable '{key}' raised [{e:?}]"),
    }
}
