use url::Url;

/// The three identity-provider endpoints a native login uses.
#[derive(Debug, Clone)]
pub struct OidcEndpoints {
    pub authorization: Url,
    pub device_authorization: Url,
    pub token: Url,
}

impl OidcEndpoints {
    /// The endpoints of a Keycloak realm, from the server's base address
    /// (`https://auth.origincloud.eu`) and the realm name. Keycloak's paths
    /// are fixed, so this saves the discovery request on every login.
    pub fn keycloak_realm(base_url: &str, realm: &str) -> Result<Self, url::ParseError> {
        let realm_base = format!(
            "{}/realms/{realm}/protocol/openid-connect",
            base_url.trim_end_matches('/')
        );
        Ok(Self {
            authorization: Url::parse(&format!("{realm_base}/auth"))?,
            device_authorization: Url::parse(&format!("{realm_base}/auth/device"))?,
            token: Url::parse(&format!("{realm_base}/token"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The paths the realm's discovery document advertises for
    /// `authorization_endpoint`, `device_authorization_endpoint` and
    /// `token_endpoint`.
    #[test]
    fn keycloak_realm_endpoints_match_the_realm_discovery_document() {
        let endpoints =
            OidcEndpoints::keycloak_realm("https://auth.origincloud.eu/", "origincloud").unwrap();

        assert_eq!(
            endpoints.authorization.as_str(),
            "https://auth.origincloud.eu/realms/origincloud/protocol/openid-connect/auth"
        );
        assert_eq!(
            endpoints.device_authorization.as_str(),
            "https://auth.origincloud.eu/realms/origincloud/protocol/openid-connect/auth/device"
        );
        assert_eq!(
            endpoints.token.as_str(),
            "https://auth.origincloud.eu/realms/origincloud/protocol/openid-connect/token"
        );
    }
}
