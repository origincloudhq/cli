use crate::identity::Urn;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum RuleArgError {
    #[error("missing required field `{0}`")]
    MissingField(&'static str),
    #[error("invalid value {value:?} for field `{field}`")]
    InvalidValue { field: &'static str, value: String },
    #[error("unknown field `{0}` (expected key=value pairs separated by commas)")]
    UnknownField(String),
    #[error("malformed key=value pair {0:?}")]
    MalformedPair(String),
    #[error("set exactly one of `ip-range`/`security-group-urn`, not {0}")]
    PeerCardinality(&'static str),
}

/// Parses a compact `key=value,key=value` rule string into a lookup map,
/// erroring on any pair that isn't `key=value`. Shared by every rule-arg
/// `FromStr` impl below - not part of the public parsing contract, just
/// avoids repeating the same split/validate logic per rule kind.
fn parse_pairs(s: &str) -> Result<HashMap<&str, &str>, RuleArgError> {
    s.split(',')
        .map(|pair| {
            pair.split_once('=')
                .ok_or_else(|| RuleArgError::MalformedPair(pair.to_string()))
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProtocolArg {
    Any,
    Tcp,
    Udp,
    Icmp,
}

impl FromStr for ProtocolArg {
    type Err = RuleArgError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "any" => Ok(Self::Any),
            "tcp" => Ok(Self::Tcp),
            "udp" => Ok(Self::Udp),
            "icmp" => Ok(Self::Icmp),
            _ => Err(RuleArgError::InvalidValue {
                field: "protocol",
                value: s.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AclDirectionArg {
    Ingress,
    Egress,
}

impl FromStr for AclDirectionArg {
    type Err = RuleArgError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ingress" => Ok(Self::Ingress),
            "egress" => Ok(Self::Egress),
            _ => Err(RuleArgError::InvalidValue {
                field: "direction",
                value: s.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AclPolicyArg {
    Accept,
    Drop,
}

impl FromStr for AclPolicyArg {
    type Err = RuleArgError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "accept" => Ok(Self::Accept),
            "drop" => Ok(Self::Drop),
            _ => Err(RuleArgError::InvalidValue {
                field: "policy",
                value: s.to_string(),
            }),
        }
    }
}

/// Parses a port spec: `"22"` -> `(Some(22), Some(22))`, `"22-100"` ->
/// `(Some(22), Some(100))`, `"any"` -> `(None, None)` - matching the API's
/// own "omit both for any port, set only `from` for an exact port, set both
/// for a range" convention (`NetworkAclPortInput`).
fn parse_port(s: &str) -> Result<(Option<u16>, Option<u16>), RuleArgError> {
    if s == "any" {
        return Ok((None, None));
    }
    let invalid = || RuleArgError::InvalidValue {
        field: "port",
        value: s.to_string(),
    };
    match s.split_once('-') {
        Some((from, to)) => {
            let from: u16 = from.parse().map_err(|_| invalid())?;
            let to: u16 = to.parse().map_err(|_| invalid())?;
            Ok((Some(from), Some(to)))
        }
        None => {
            let port: u16 = s.parse().map_err(|_| invalid())?;
            Ok((Some(port), Some(port)))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AclRuleArg {
    pub priority: u16,
    pub direction: AclDirectionArg,
    pub protocol: ProtocolArg,
    pub port_from: Option<u16>,
    pub port_to: Option<u16>,
    pub source: String,
    pub policy: AclPolicyArg,
}

impl FromStr for AclRuleArg {
    type Err = RuleArgError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pairs = parse_pairs(s)?;

        let get = |field: &'static str| {
            pairs
                .get(field)
                .copied()
                .ok_or(RuleArgError::MissingField(field))
        };

        let priority: u16 = get("priority")?
            .parse()
            .map_err(|_| RuleArgError::InvalidValue {
                field: "priority",
                value: get("priority").unwrap_or_default().to_string(),
            })?;
        let direction = get("direction")?.parse()?;
        let protocol = get("protocol")?.parse()?;
        let (port_from, port_to) = parse_port(get("port")?)?;
        let source = get("source")?.to_string();
        let policy = get("policy")?.parse()?;

        for key in pairs.keys() {
            if !matches!(
                *key,
                "priority" | "direction" | "protocol" | "port" | "source" | "policy"
            ) {
                return Err(RuleArgError::UnknownField(key.to_string()));
            }
        }

        Ok(Self {
            priority,
            direction,
            protocol,
            port_from,
            port_to,
            source,
            policy,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SecurityGroupRuleArg {
    pub protocol: ProtocolArg,
    pub port_from: Option<u16>,
    pub port_to: Option<u16>,
    pub ip_range: Option<String>,
    pub security_group_urn: Option<Urn>,
    pub description: Option<String>,
}

impl FromStr for SecurityGroupRuleArg {
    type Err = RuleArgError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pairs = parse_pairs(s)?;

        let get = |field: &'static str| {
            pairs
                .get(field)
                .copied()
                .ok_or(RuleArgError::MissingField(field))
        };

        let protocol = get("protocol")?.parse()?;
        let (port_from, port_to) = parse_port(get("port")?)?;
        let ip_range = pairs.get("ip-range").map(|v| v.to_string());
        let security_group_urn = pairs
            .get("security-group-urn")
            .map(|v| {
                v.parse::<Urn>().map_err(|_| RuleArgError::InvalidValue {
                    field: "security-group-urn",
                    value: v.to_string(),
                })
            })
            .transpose()?;
        let description = pairs.get("description").map(|v| v.to_string());

        match (&ip_range, &security_group_urn) {
            (Some(_), Some(_)) => return Err(RuleArgError::PeerCardinality("both")),
            (None, None) => return Err(RuleArgError::PeerCardinality("neither")),
            _ => {}
        }

        for key in pairs.keys() {
            if !matches!(
                *key,
                "protocol" | "port" | "ip-range" | "security-group-urn" | "description"
            ) {
                return Err(RuleArgError::UnknownField(key.to_string()));
            }
        }

        Ok(Self {
            protocol,
            port_from,
            port_to,
            ip_range,
            security_group_urn,
            description,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_full_acl_rule() {
        let rule: AclRuleArg =
            "priority=100,direction=ingress,protocol=tcp,port=22,source=0.0.0.0/0,policy=accept"
                .parse()
                .unwrap();
        assert_eq!(
            rule,
            AclRuleArg {
                priority: 100,
                direction: AclDirectionArg::Ingress,
                protocol: ProtocolArg::Tcp,
                port_from: Some(22),
                port_to: Some(22),
                source: "0.0.0.0/0".to_string(),
                policy: AclPolicyArg::Accept,
            }
        );
    }

    #[test]
    fn acl_rule_missing_required_field_errors() {
        let err: RuleArgError =
            "priority=100,direction=ingress,protocol=tcp,port=22,source=0.0.0.0/0"
                .parse::<AclRuleArg>()
                .unwrap_err();
        assert_eq!(err, RuleArgError::MissingField("policy"));
    }

    #[test]
    fn acl_rule_invalid_enum_value_errors() {
        let err: RuleArgError =
            "priority=100,direction=sideways,protocol=tcp,port=22,source=0.0.0.0/0,policy=accept"
                .parse::<AclRuleArg>()
                .unwrap_err();
        assert_eq!(
            err,
            RuleArgError::InvalidValue {
                field: "direction",
                value: "sideways".to_string(),
            }
        );
    }

    #[test]
    fn acl_rule_unknown_field_errors() {
        let err: RuleArgError = "priority=100,direction=ingress,protocol=tcp,port=22,source=0.0.0.0/0,policy=accept,bogus=1"
            .parse::<AclRuleArg>()
            .unwrap_err();
        assert_eq!(err, RuleArgError::UnknownField("bogus".to_string()));
    }

    #[test]
    fn port_formats_parse_correctly() {
        assert_eq!(parse_port("22").unwrap(), (Some(22), Some(22)));
        assert_eq!(parse_port("22-100").unwrap(), (Some(22), Some(100)));
        assert_eq!(parse_port("any").unwrap(), (None, None));
        assert!(parse_port("not-a-port").is_err());
    }

    #[test]
    fn parses_a_security_group_rule_with_ip_range_peer() {
        let rule: SecurityGroupRuleArg = "protocol=tcp,port=22,ip-range=0.0.0.0/0,description=ssh"
            .parse()
            .unwrap();
        assert_eq!(
            rule,
            SecurityGroupRuleArg {
                protocol: ProtocolArg::Tcp,
                port_from: Some(22),
                port_to: Some(22),
                ip_range: Some("0.0.0.0/0".to_string()),
                security_group_urn: None,
                description: Some("ssh".to_string()),
            }
        );
    }

    #[test]
    fn security_group_rule_requires_exactly_one_peer_field() {
        let err = "protocol=tcp,port=22"
            .parse::<SecurityGroupRuleArg>()
            .unwrap_err();
        assert_eq!(err, RuleArgError::PeerCardinality("neither"));

        let err = "protocol=tcp,port=22,ip-range=0.0.0.0/0,security-group-urn=urn:origin:cloud:network::0f1e2d3c4b5a:other-project-456:sg-7RkjXPm2UGc"
            .parse::<SecurityGroupRuleArg>()
            .unwrap_err();
        assert_eq!(err, RuleArgError::PeerCardinality("both"));
    }

    #[test]
    fn security_group_rule_any_port_supported() {
        let rule: SecurityGroupRuleArg =
            "protocol=any,port=any,ip-range=0.0.0.0/0".parse().unwrap();
        assert_eq!(rule.port_from, None);
        assert_eq!(rule.port_to, None);
    }

    #[test]
    fn icmp_is_accepted_wherever_tcp_and_udp_are() {
        let sg: SecurityGroupRuleArg = "protocol=icmp,port=any,ip-range=::/0".parse().unwrap();
        assert_eq!(sg.protocol, ProtocolArg::Icmp);
        assert_eq!((sg.port_from, sg.port_to), (None, None));

        let acl: AclRuleArg =
            "priority=100,direction=ingress,protocol=icmp,port=any,source=0.0.0.0/0,policy=accept"
                .parse()
                .unwrap();
        assert_eq!(acl.protocol, ProtocolArg::Icmp);
    }
}
