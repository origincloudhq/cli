use std::time::Duration;

/// Why a login or a token refresh did not produce tokens.
///
/// Every message is written for the person at the terminal: it says what
/// happened and, where there is one, what to do next.
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("could not reach the identity provider: {0}")]
    Transport(#[from] reqwest::Error),

    /// An OAuth error response (`error`, `error_description`) from the
    /// identity provider's token or device endpoint.
    #[error("the identity provider refused the request: {}", describe(error, description.as_deref()))]
    Refused {
        error: String,
        description: Option<String>,
    },

    #[error("unexpected response from the identity provider (HTTP {status}): {body}")]
    UnexpectedResponse { status: u16, body: String },

    /// The browser came back with an OAuth `error` parameter, for example
    /// because the person cancelled at the login page.
    #[error("the login did not complete in the browser: {}", describe(error, description.as_deref()))]
    AuthorizationFailed {
        error: String,
        description: Option<String>,
    },

    /// The callback's `state` is missing or is not the one this login sent,
    /// so the response belongs to some other login attempt (or a forged one)
    /// and its code must not be redeemed.
    #[error("the browser returned a login response that does not belong to this login attempt; start the login again")]
    StateMismatch,

    #[error("the browser returned an incomplete login response: {0}")]
    MalformedCallback(String),

    #[error("no login completed within {} seconds; start the login again", .0.as_secs())]
    TimedOut(Duration),

    #[error("the device code expired before the login was approved; start the login again")]
    DeviceCodeExpired,

    #[error("the login was denied on the approving device")]
    AccessDenied,

    #[error("could not listen for the browser's login response on 127.0.0.1: {0}")]
    Listener(#[source] std::io::Error),

    #[error("invalid identity provider address: {0}")]
    InvalidEndpoint(#[from] url::ParseError),
}

fn describe(error: &str, description: Option<&str>) -> String {
    match description {
        Some(description) if !description.is_empty() => format!("{description} ({error})"),
        _ => error.to_string(),
    }
}
