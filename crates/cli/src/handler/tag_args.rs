//! `--tag KEY=VALUE`, shared by every command that sets tags. Only the
//! shape is checked here; what a key or value may contain is the server's to
//! decide, and its error says what was wrong.

use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TagArg {
    pub key: String,
    pub value: String,
}

impl FromStr for TagArg {
    type Err = String;

    /// Splits at the first `=`, so a value may contain `=` and a key may
    /// not. An empty value (`flag=`) is a tag with no value.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (key, value) = s
            .split_once('=')
            .ok_or_else(|| format!("expected KEY=VALUE, got {s:?}"))?;
        if key.is_empty() {
            return Err(format!("expected KEY=VALUE, got {s:?}: the key is empty"));
        }
        Ok(TagArg {
            key: key.to_string(),
            value: value.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_the_first_equals_sign() {
        assert_eq!(
            "origin:name=a=b".parse::<TagArg>(),
            Ok(TagArg {
                key: "origin:name".to_string(),
                value: "a=b".to_string()
            })
        );
        assert_eq!(
            "flag=".parse::<TagArg>(),
            Ok(TagArg {
                key: "flag".to_string(),
                value: String::new()
            })
        );
    }

    #[test]
    fn refuses_a_missing_separator_or_key() {
        assert!("origin:name".parse::<TagArg>().is_err());
        assert!("=value".parse::<TagArg>().is_err());
    }
}
