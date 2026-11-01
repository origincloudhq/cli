//! Interactive OAuth login for the platform's command-line tools.
//!
//! Two ways for a person at a terminal to get tokens without the tool ever
//! seeing their password:
//!
//! - [`NativeLoginClient::login_in_browser`]: Authorization Code with PKCE
//!   (`S256`) and a loopback redirect to `127.0.0.1` on an OS-assigned port
//!   (RFC 7636, RFC 8252).
//! - [`NativeLoginClient::login_with_device_code`]: the Device Authorization
//!   Grant (RFC 8628), for a host with no browser, such as an SSH session.
//!
//! Whatever the identity provider offers in its own login page (password,
//! social login, passkeys, a second factor) works through both, because the
//! person authenticates there and not here.
//!
//! The crate returns a [`TokenResponse`] and stores nothing: where a session
//! lives and how it is named is each tool's own concern. Prompts go through
//! [`LoginProgress`]; [`StderrPrompts`] is the terminal version.

mod browser;
mod client;
mod device;
mod endpoints;
mod error;
#[cfg(test)]
mod flow_tests;
mod identity;
mod pkce;
mod progress;
mod stderr_prompts;
mod token;

pub use browser::BrowserLoginOptions;
pub use client::NativeLoginClient;
pub use device::DeviceCodePrompt;
pub use endpoints::OidcEndpoints;
pub use error::LoginError;
pub use identity::display_identity;
pub use progress::LoginProgress;
pub use stderr_prompts::StderrPrompts;
pub use token::TokenResponse;
