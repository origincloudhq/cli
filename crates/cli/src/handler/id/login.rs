use crate::client_auth::{login, login_with_token, LoginScheme};
use crate::graphql::TokenProvider;
use crate::output::{CommandResult, SerializableOutput};
use crate::service_apis::iam::IamServiceClient;
use origincloud_oauth_login::display_identity;
use std::fmt::Display;
use std::future::Future;
use url::Url;

/// `default_username` from the profile no longer logs anyone in; it pre-fills
/// the username on the browser login page, where it can still be changed.
pub(super) async fn perform_login(
    device: bool,
    default_username: Option<String>,
    api_url: &Url,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let scheme = if device {
        LoginScheme::Device
    } else {
        LoginScheme::Browser {
            login_hint: default_username,
        }
    };

    let session = match login(scheme).await {
        Ok(session) => session,
        Err(err) => {
            return Ok(Box::new(CommandResult::Failure(format!(
                "login failed: {err}"
            ))))
        }
    };

    // On stderr, beside the login prompts: stdout keeps the same result
    // envelope it has always had, which scripts parse.
    match display_identity(session.token()) {
        Some(identity) => eprintln!("Logged in as {identity}."),
        None => eprintln!("Logged in."),
    }

    let iam = IamServiceClient::new(api_url.clone());
    if let Some(warning) = resolve_first_authentication(|| iam.current_user()).await {
        eprintln!("{warning}");
    }

    Ok(Box::new(CommandResult::Success))
}

/// Logs in as the service account the identity provider's token in
/// `token_file` assumes. A service account has no user to read, and no
/// invites to apply.
pub(super) async fn perform_token_login(
    token_file: &str,
    service_account: Option<String>,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    match login_with_token(token_file, service_account).await {
        Ok(session) => {
            if let Some(account) = session.service_account() {
                eprintln!("Logged in as {account}.");
            }
            Ok(Box::new(CommandResult::Success))
        }
        Err(err) => Ok(Box::new(CommandResult::Failure(format!(
            "login failed: {err:#}"
        )))),
    }
}

/// Reads the signed-in user once, right after a user login.
///
/// The platform applies a user's pending organization invites on their first
/// `currentUser` read, not at login. The console makes that read on every
/// sign-in; without this one, a person who only uses the CLI would get no
/// invited role until they signed in to the console.
///
/// The login has already succeeded and its session is stored, so a failed read
/// does not fail it. Returns the warning to print instead, which says how to
/// apply the invites later. The warning says "may": the server
/// applies invites before it looks the user up, so a read can fail after they
/// were applied.
async fn resolve_first_authentication<F, Fut, T, E>(read_current_user: F) -> Option<String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: Display,
{
    match read_current_user().await {
        Ok(_) => None,
        Err(err) => Some(format!(
            "warning: could not read your user after logging in, so pending organization \
             invites for your email address may not be applied yet ({err}). \
             Log in again, or sign in to the console, to apply them."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_first_authentication;
    use std::cell::Cell;

    #[tokio::test]
    async fn a_successful_read_prints_nothing() {
        let reads = Cell::new(0);
        let warning = resolve_first_authentication(|| async {
            reads.set(reads.get() + 1);
            Ok::<_, String>(())
        })
        .await;

        assert_eq!(warning, None);
        assert_eq!(reads.get(), 1);
    }

    #[tokio::test]
    async fn a_failed_read_warns_with_the_cause_and_how_to_apply_invites() {
        let warning = resolve_first_authentication(|| async {
            Err::<(), _>("transport: connection refused")
        })
        .await
        .expect("a failed read must produce a warning");

        assert!(warning.starts_with("warning: "), "{warning}");
        assert!(
            warning.contains("transport: connection refused"),
            "{warning}"
        );
        assert!(
            warning.contains("Log in again, or sign in to the console, to apply them."),
            "{warning}"
        );
    }
}
