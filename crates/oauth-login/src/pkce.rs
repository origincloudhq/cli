use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

/// The only code-challenge method this crate sends. `plain` exists in the
/// spec for clients that cannot hash, and offers no protection against an
/// intercepted authorization request, so it is never used.
pub(crate) const CODE_CHALLENGE_METHOD: &str = "S256";

/// A PKCE code verifier and its `S256` challenge (RFC 7636).
pub(crate) struct Pkce {
    verifier: String,
    challenge: String,
}

impl Pkce {
    /// 32 random bytes, base64url-encoded: a 43-character verifier, the
    /// minimum length RFC 7636 allows and 256 bits of entropy.
    pub(crate) fn generate() -> Self {
        Self::from_verifier(random_urlsafe_token())
    }

    fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }

    pub(crate) fn verifier(&self) -> &str {
        &self.verifier
    }

    pub(crate) fn challenge(&self) -> &str {
        &self.challenge
    }
}

/// An unguessable value for the authorization request's `state`, which ties
/// the browser's callback to this login attempt.
pub(crate) fn random_state() -> String {
    random_urlsafe_token()
}

fn random_urlsafe_token() -> String {
    let bytes: [u8; 32] = rand::random();
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 7636, Appendix B: the worked example's verifier and challenge.
    #[test]
    fn the_challenge_is_the_rfc_7636_s256_transform_of_the_verifier() {
        let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string());
        assert_eq!(
            pkce.challenge(),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn a_generated_verifier_is_43_unreserved_characters_and_never_repeats() {
        let first = Pkce::generate();
        let second = Pkce::generate();

        assert_eq!(first.verifier().len(), 43);
        assert!(first
            .verifier()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_ne!(first.verifier(), second.verifier());
        assert_ne!(random_state(), random_state());
    }
}
