use crate::device::DeviceCodePrompt;
use url::Url;

/// What a login has to show the person while it waits for them.
///
/// The login flows never print on their own: the caller passes the prompts
/// to use. [`crate::StderrPrompts`] writes them to stderr, so a CLI's stdout
/// stays its JSON result.
pub trait LoginProgress: Send + Sync {
    /// The browser login's authorization URL. `browser_opened` is whether a
    /// browser was launched; the URL is shown either way, since a launch that
    /// "succeeds" can still open nothing (no display, a misconfigured
    /// handler).
    fn show_authorization_url(&self, url: &Url, browser_opened: bool);

    /// The device login's verification address and user code.
    fn show_device_code(&self, prompt: &DeviceCodePrompt);
}
