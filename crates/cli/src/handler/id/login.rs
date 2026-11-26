use crate::client_auth::{login, login_with_token, LoginScheme};
use crate::config::ClientProfile;
use crate::output::{CommandResult, SerializableOutput};
use crate::sdk::credentials::describe;
use crate::sdk::session_client;
use crate::token::TokenProvider;
use origincloud_core::Client;
use origincloud_iam::v1beta1::{ActivatePendingRoleBindingsRequest, UserClient};
use origincloud_oauth_login::display_identity;
use std::fmt::Display;
use std::future::Future;

/// `default_username` from the profile no longer logs anyone in; it pre-fills
/// the username on the browser login page, where it can still be changed.
pub(super) async fn perform_login(
    profile: &ClientProfile,
    device: bool,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let scheme = if device {
        LoginScheme::Device
    } else {
        LoginScheme::Browser {
            login_hint: profile.default_username.clone(),
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

    let activate = || activate_pending_role_bindings(profile, session.token());
    if let Some(warning) = resolve_first_authentication(activate).await {
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

/// Activates the role bindings pending for the email address of the person
/// who has just signed in, as they are: IAM's
/// `users/me:activatePendingRoleBindings`, with their new session's token.
async fn activate_pending_role_bindings(
    profile: &ClientProfile,
    token: &str,
) -> anyhow::Result<()> {
    activate_with(&session_client(profile, "iam", token)?).await
}

/// [`activate_pending_role_bindings`] with `client`.
async fn activate_with(client: &Client) -> anyhow::Result<()> {
    UserClient::new(client)
        .activate_pending_role_bindings(ActivatePendingRoleBindingsRequest {
            name: "users/me".to_owned(),
        })
        .await
        .map_err(|err| anyhow::anyhow!(describe(&err)))?;
    Ok(())
}

/// Activates the role bindings pending for the person's email address once,
/// right after a user login.
///
/// The platform grants the roles a person was invited to only once they
/// ask for them, signed in with the address the invitation named, which
/// the console does on every sign-in; without this call, a person who only
/// uses the CLI would get no invited role until they signed in to the
/// console.
///
/// The login has already succeeded and its session is stored, so a failed
/// call does not fail it. Returns the warning to print instead, which says
/// how to activate the bindings later. The warning says "may": a call can
/// fail after the server activated them.
async fn resolve_first_authentication<F, Fut, T, E>(activate: F) -> Option<String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: Display,
{
    match activate().await {
        Ok(_) => None,
        Err(err) => Some(format!(
            "warning: could not activate the role bindings pending for your email address \
             after logging in, so the roles you were invited to may not be granted yet \
             ({err}). Log in again, or sign in to the console, to activate them."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{activate_with, resolve_first_authentication};
    use fake_server::{FakeServer, Received, Response};
    use origincloud_core::credentials::AccessToken;
    use origincloud_core::Client;
    use std::cell::Cell;
    use std::sync::{Arc, Mutex};

    /// IAM at a local address, answering every call with `status`, and the
    /// calls it received.
    async fn iam(status: u16) -> (FakeServer, Client, Arc<Mutex<Vec<Received>>>) {
        let received = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&received);
        let server = FakeServer::start(move |r: Received| {
            seen.lock().unwrap().push(r);
            let response = match status {
                200 => Response::json(r#"{"name":"users/u-1","email":"a@example.com"}"#),
                _ => Response::json(
                    r#"{"error":{"code":400,"message":"a service account has no pending role bindings","status":"INVALID_ARGUMENT"}}"#,
                )
                .with_status(status),
            };
            async move { response }
        })
        .await;
        let client = Client::builder()
            .credentials(AccessToken::new("session-token"))
            .impersonate_service_account("")
            .endpoint(server.url())
            .max_attempts(1)
            .build()
            .unwrap();
        (server, client, received)
    }

    #[tokio::test]
    async fn the_login_activates_the_caller_s_pending_role_bindings_with_its_session() {
        let (_server, client, received) = iam(200).await;

        activate_with(&client).await.unwrap();

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 1);
        let call = &received[0];
        assert_eq!(call.method, "POST");
        assert_eq!(call.path, "/v1beta1/users/me:activatePendingRoleBindings");
        assert_eq!(call.header("authorization"), Some("Bearer session-token"));
    }

    #[tokio::test]
    async fn a_refused_activation_is_the_api_s_message() {
        let (_server, client, _) = iam(400).await;

        let err = activate_with(&client).await.unwrap_err().to_string();

        assert!(
            err.contains("a service account has no pending role bindings"),
            "{err}"
        );
        assert!(!err.starts_with("origincloud: "), "{err}");
    }

    #[tokio::test]
    async fn a_successful_activation_prints_nothing() {
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
    async fn a_failed_activation_warns_with_the_cause_and_how_to_retry() {
        let warning = resolve_first_authentication(|| async {
            Err::<(), _>("transport: connection refused")
        })
        .await
        .expect("a failed activation must produce a warning");

        assert!(warning.starts_with("warning: "), "{warning}");
        assert!(
            warning.contains("transport: connection refused"),
            "{warning}"
        );
        assert!(
            warning.contains("Log in again, or sign in to the console, to activate them."),
            "{warning}"
        );
    }
}
