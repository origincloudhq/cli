use crate::config::ClientSession;
use crate::graphql::TokenProvider;
use crate::output::{CommandResult, SerializableOutput, SerializationError};
use serde::Serialize;

pub(super) async fn perform_token(
    as_header: bool,
    raw: bool,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let credentials = match ClientSession::read_and_validate_credentials().await {
        None => {
            return Ok(Box::new(CommandResult::Failure(
                "unauthenticated".to_string(),
            )))
        }
        Some(credentials) => credentials,
    };

    if as_header {
        Ok(Box::new(TokenHeaderOutput {
            token: credentials.token().to_string(),
            ty: credentials.ty().to_string(),
            raw,
        }))
    } else {
        Ok(Box::new(TokenOutput {
            token: credentials.token().to_string(),
            raw,
        }))
    }
}

#[derive(Serialize)]
struct TokenOutput {
    token: String,
    #[serde(skip)]
    raw: bool,
}

impl SerializableOutput for TokenOutput {
    fn to_json(&self) -> Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|e| SerializationError::Other(e.to_string()))
    }

    fn to_raw(&self) -> Option<String> {
        self.raw.then(|| self.token.clone())
    }
}

#[derive(Serialize)]
struct TokenHeaderOutput {
    token: String,
    ty: String,
    #[serde(skip)]
    raw: bool,
}

impl SerializableOutput for TokenHeaderOutput {
    fn to_json(&self) -> Result<serde_json::Value, SerializationError> {
        let mut map = serde_json::Map::new();
        map.insert(
            String::from("Authorization"),
            serde_json::Value::String(format!("{} {}", self.ty, self.token)),
        );

        Ok(serde_json::Value::Object(map))
    }

    fn to_raw(&self) -> Option<String> {
        self.raw
            .then(|| format!("Authorization: {} {}", self.ty, self.token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_output_to_raw_is_none_without_raw_flag() {
        let output = TokenOutput {
            token: "abc123".to_string(),
            raw: false,
        };
        assert_eq!(output.to_raw(), None);
    }

    #[test]
    fn token_output_to_raw_is_bare_token() {
        let output = TokenOutput {
            token: "abc123".to_string(),
            raw: true,
        };
        assert_eq!(output.to_raw(), Some("abc123".to_string()));
    }

    #[test]
    fn token_output_to_json_is_unchanged_by_raw_flag() {
        let output = TokenOutput {
            token: "abc123".to_string(),
            raw: true,
        };
        assert_eq!(
            output.to_json().unwrap(),
            serde_json::json!({ "token": "abc123" })
        );
    }

    #[test]
    fn token_header_output_to_raw_is_curl_ready_header_line() {
        let output = TokenHeaderOutput {
            token: "abc123".to_string(),
            ty: "Bearer".to_string(),
            raw: true,
        };
        assert_eq!(
            output.to_raw(),
            Some("Authorization: Bearer abc123".to_string())
        );
    }

    #[test]
    fn token_header_output_to_raw_is_none_without_raw_flag() {
        let output = TokenHeaderOutput {
            token: "abc123".to_string(),
            ty: "Bearer".to_string(),
            raw: false,
        };
        assert_eq!(output.to_raw(), None);
    }

    #[test]
    fn token_header_output_to_json_is_unchanged_by_raw_flag() {
        let output = TokenHeaderOutput {
            token: "abc123".to_string(),
            ty: "Bearer".to_string(),
            raw: true,
        };
        assert_eq!(
            output.to_json().unwrap(),
            serde_json::json!({ "Authorization": "Bearer abc123" })
        );
    }
}
