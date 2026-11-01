use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

/// Who an access token was issued to, for telling the person who they are
/// now logged in as: `preferred_username`, else `email`, else `sub`.
///
/// For display only. The signature is not checked - the token came straight
/// from the token endpoint over TLS, and nothing is decided on this value.
/// `None` if the token is not a JWT or names nobody.
pub fn display_identity(access_token: &str) -> Option<String> {
    let payload = access_token.split('.').nth(1)?;
    let claims: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    ["preferred_username", "email", "sub"]
        .iter()
        .find_map(|claim| claims.get(claim)?.as_str().filter(|v| !v.is_empty()))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jwt(claims: &str) -> String {
        format!(
            "eyJhbGciOiJSUzI1NiJ9.{}.sig",
            URL_SAFE_NO_PAD.encode(claims)
        )
    }

    #[test]
    fn prefers_the_username_then_the_email_then_the_subject() {
        assert_eq!(
            display_identity(&jwt(
                r#"{"sub":"u-1","email":"a@b.c","preferred_username":"ada"}"#
            )),
            Some("ada".to_string())
        );
        assert_eq!(
            display_identity(&jwt(r#"{"sub":"u-1","email":"a@b.c"}"#)),
            Some("a@b.c".to_string())
        );
        assert_eq!(
            display_identity(&jwt(r#"{"sub":"u-1","preferred_username":""}"#)),
            Some("u-1".to_string())
        );
    }

    #[test]
    fn an_opaque_or_anonymous_token_names_nobody() {
        assert_eq!(display_identity("opaque-token"), None);
        assert_eq!(display_identity(&jwt("{}")), None);
        assert_eq!(display_identity("a.!!!.c"), None);
    }
}
