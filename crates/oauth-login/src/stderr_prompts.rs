use crate::device::DeviceCodePrompt;
use crate::progress::LoginProgress;
use url::Url;

/// The terminal prompts both CLIs show during a login, written to stderr so
/// stdout carries only the command's result.
///
/// `program` is the binary's name, used in the hint that points a person
/// without a browser at `<program> id login --device`.
pub struct StderrPrompts {
    program: &'static str,
}

impl StderrPrompts {
    pub fn new(program: &'static str) -> Self {
        Self { program }
    }

    fn authorization_url_text(&self, url: &Url, browser_opened: bool) -> String {
        let lead = if browser_opened {
            "Opening your browser to log in. If it does not open, visit:"
        } else {
            "Open this address in a browser to log in:"
        };
        format!(
            "{lead}\n\n    {url}\n\nWaiting for the login to complete. On a machine without a \
             browser, use `{} id login --device` instead.\n",
            self.program
        )
    }

    fn device_code_text(prompt: &DeviceCodePrompt) -> String {
        let (address, instruction) = match &prompt.verification_uri_complete {
            Some(complete) => (
                complete.as_str(),
                format!("and check that it shows the code {}.", prompt.user_code),
            ),
            None => (
                prompt.verification_uri.as_str(),
                format!("and enter the code {}.", prompt.user_code),
            ),
        };
        format!(
            "To log in, open this address on any device:\n\n    {address}\n\n{instruction}\n\
             Waiting for approval (the code expires in {} minutes).\n",
            prompt.expires_in.as_secs().div_ceil(60)
        )
    }
}

impl LoginProgress for StderrPrompts {
    fn show_authorization_url(&self, url: &Url, browser_opened: bool) {
        eprint!("{}", self.authorization_url_text(url, browser_opened));
    }

    fn show_device_code(&self, prompt: &DeviceCodePrompt) {
        eprint!("{}", Self::device_code_text(prompt));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn prompt(complete: Option<&str>) -> DeviceCodePrompt {
        DeviceCodePrompt {
            user_code: "WXYZ-1234".into(),
            verification_uri: "https://idp/device".into(),
            verification_uri_complete: complete.map(str::to_string),
            expires_in: Duration::from_secs(599),
        }
    }

    #[test]
    fn the_browser_prompt_shows_the_address_and_names_this_program_s_device_login() {
        let url = Url::parse("https://idp/auth?state=s").unwrap();
        let text = StderrPrompts::new("originop").authorization_url_text(&url, false);

        assert!(text.starts_with("Open this address in a browser"));
        assert!(text.contains("    https://idp/auth?state=s\n"));
        assert!(text.contains("`originop id login --device`"));

        let opened = StderrPrompts::new("origin").authorization_url_text(&url, true);
        assert!(opened.starts_with("Opening your browser"));
        assert!(opened.contains("https://idp/auth?state=s"));
    }

    #[test]
    fn a_prefilled_address_asks_to_check_the_code_a_bare_one_to_enter_it() {
        let prefilled = StderrPrompts::device_code_text(&prompt(Some(
            "https://idp/device?user_code=WXYZ-1234",
        )));
        assert!(prefilled.contains("    https://idp/device?user_code=WXYZ-1234\n"));
        assert!(prefilled.contains("check that it shows the code WXYZ-1234"));

        let bare = StderrPrompts::device_code_text(&prompt(None));
        assert!(bare.contains("    https://idp/device\n"));
        assert!(bare.contains("enter the code WXYZ-1234"));
        assert!(bare.contains("expires in 10 minutes"));
    }
}
