#[derive(thiserror::Error, Debug)]
pub enum SerializationError {
    #[error("other: {0}")]
    Other(String),
}

pub trait SerializableOutput {
    fn to_json(&self) -> Result<serde_json::Value, SerializationError>;

    fn exit_code(&self) -> i32 {
        0
    }

    /// Plain-text rendering that bypasses the JSON envelope, for commands that
    /// support a `--raw` output mode. Returning `None` (the default) keeps the
    /// existing JSON rendering.
    fn to_raw(&self) -> Option<String> {
        None
    }
}

#[derive(serde::Serialize)]
#[serde(tag = "code", content = "description", rename_all = "snake_case")]
pub enum CommandResult {
    Success,
    /// The rendered failure description.
    ///
    /// The envelope's `code` stays the fixed literal `"failure"` for
    /// backwards compatibility - it has never been the server's error code,
    /// and scripts parse it. The server's stable code and correlation id are
    /// carried inside the description by the clients' rendering, so they
    /// reach the user without changing the envelope's
    /// shape.
    Failure(String),
}

impl SerializableOutput for CommandResult {
    fn to_json(&self) -> Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|e| SerializationError::Other(e.to_string()))
    }

    fn exit_code(&self) -> i32 {
        match self {
            CommandResult::Success => 0,
            CommandResult::Failure(_) => 1,
        }
    }
}
