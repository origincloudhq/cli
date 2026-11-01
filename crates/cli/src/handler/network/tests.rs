//! Argument-surface tests for the network subcommands.
//!
//! These parse through the real root command, so they pin the invocation a
//! user actually types - not just the shape of the enum.

use crate::handler::network::NetworkCommands;
use crate::root_command::{Cli, Commands};
use clap::Parser;

const DNS_POLICY_URN: &str =
    "urn:origin:cloud:network:eu-central1:0a1b2c3d4e5f:my-project-123:dnspolicy/abc123";
const PROJECT_URN: &str = "urn:origin:cloud:admin:eu-central1:0a1b2c3d4e5f::my-project-123";
const SUBNET_URN: &str =
    "urn:origin:cloud:network:eu-central1:0a1b2c3d4e5f:my-project-123:subnet/abc123";
const VPC_URN: &str = "urn:origin:cloud:network:eu-central1:0a1b2c3d4e5f:my-project-123:vpc/abc123";
const ACL_URN: &str = "urn:origin:cloud:network:eu-central1:0a1b2c3d4e5f:my-project-123:acl/abc123";
const SECURITY_GROUP_URN: &str =
    "urn:origin:cloud:network:eu-central1:0a1b2c3d4e5f:my-project-123:sg/abc123";

const ACL_RULE: &str =
    "priority=1,direction=ingress,protocol=tcp,port=22,source=0.0.0.0/0,policy=accept";
const OTHER_ACL_RULE: &str =
    "priority=2,direction=egress,protocol=any,port=any,source=0.0.0.0/0,policy=accept";
const SECURITY_GROUP_RULE: &str = "protocol=tcp,port=22,ip-range=0.0.0.0/0,description=ssh";

fn parse_network(args: &[&str]) -> Result<NetworkCommands, clap::Error> {
    let mut argv = vec!["origin", "network"];
    argv.extend_from_slice(args);

    match Cli::try_parse_from(argv)?.command {
        Some(Commands::Network(command)) => Ok(command),
        _ => panic!("expected a network subcommand"),
    }
}

/// `NetworkCommands` deliberately has no `Debug` impl, so `expect_err` is
/// not available on the parse result.
fn parse_network_err(args: &[&str], why: &str) -> clap::Error {
    match parse_network(args) {
        Ok(_) => panic!("{why}"),
        Err(err) => err,
    }
}

/// `update-dns-policy` once declared `--relay-enabled`
/// as a bare flag, so this invocation - which reads as "just change the
/// upstream servers" - silently disabled relaying on the policy.
#[test]
fn update_dns_policy_refuses_to_run_without_an_explicit_relay_enabled_value() {
    let err = parse_network_err(
        &[
            "update-dns-policy",
            "--dns-policy-urn",
            DNS_POLICY_URN,
            "--upstream-server",
            "1.1.1.1",
        ],
        "omitting --relay-enabled must be an error, never an implicit false",
    );

    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

/// A bare `--relay-enabled` (the old spelling) must fail loudly rather
/// than keep meaning `true`, so an old script cannot half-work.
#[test]
fn update_dns_policy_refuses_a_bare_relay_enabled_flag() {
    let err = parse_network_err(
        &[
            "update-dns-policy",
            "--dns-policy-urn",
            DNS_POLICY_URN,
            "--no-upstream-servers",
            "--relay-enabled",
        ],
        "a bare --relay-enabled must be an error",
    );

    assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
}

#[test]
fn update_dns_policy_takes_an_explicit_relay_enabled_value() {
    for (given, expected) in [("true", true), ("false", false)] {
        let command = parse_network(&[
            "update-dns-policy",
            "--dns-policy-urn",
            DNS_POLICY_URN,
            "--relay-enabled",
            given,
            "--upstream-server",
            "1.1.1.1",
        ])
        .expect("an explicit value must parse");

        let NetworkCommands::UpdateDnsPolicy {
            dns_policy_urn,
            relay_enabled,
            upstream_servers,
            ..
        } = command
        else {
            panic!("expected update-dns-policy");
        };

        assert_eq!(dns_policy_urn.to_string(), DNS_POLICY_URN);
        assert_eq!(relay_enabled, expected);
        assert_eq!(upstream_servers, vec!["1.1.1.1".to_string()]);
    }
}

/// `create-dns-policy` is far less dangerous (there is no prior value to
/// clobber), but it stays spelled the same way as the update so that one
/// invocation cannot teach a habit the other punishes.
#[test]
fn create_dns_policy_refuses_to_run_without_an_explicit_relay_enabled_value() {
    let err = parse_network_err(
        &[
            "create-dns-policy",
            "--project-urn",
            PROJECT_URN,
            "--upstream-server",
            "1.1.1.1",
        ],
        "omitting --relay-enabled must be an error, never an implicit false",
    );

    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

#[test]
fn create_dns_policy_takes_an_explicit_relay_enabled_value() {
    let command = parse_network(&[
        "create-dns-policy",
        "--project-urn",
        PROJECT_URN,
        "--relay-enabled",
        "false",
        "--no-upstream-servers",
    ])
    .expect("an explicit value must parse");

    let NetworkCommands::CreateDnsPolicy { relay_enabled, .. } = command else {
        panic!("expected create-dns-policy");
    };

    assert!(!relay_enabled);
}

/// The precedent of the explicit boolean. Pinned here so the two
/// boolean-valued update commands cannot drift apart again.
#[test]
fn update_subnet_auto_assign_public_ip_refuses_to_run_without_an_explicit_value() {
    let err = parse_network_err(
        &[
            "update-subnet-auto-assign-public-ip",
            "--subnet-urn",
            SUBNET_URN,
        ],
        "omitting --auto-assign-public-ip must be an error",
    );

    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

// ---------------------------------------------------------------------------
// A wholesale-replace update's list-valued argument
//
// Every command below replaces a list in full, so passing none of the
// repeatable argument used to mean "replace it with the empty set" while
// reading like "leave it alone". Each list is now paired with a `--no-<list>`
// flag in a required, mutually-exclusive clap group: stating neither fails at
// parse time, and the (legitimate) empty set is still reachable by name.
// ---------------------------------------------------------------------------

/// The worst instance of the list case: this reads as a no-op and used to
/// empty a Network ACL, leaving it deny-by-default.
#[test]
fn update_acl_rules_refuses_to_run_without_a_stated_rule_set() {
    let err = parse_network_err(
        &["update-acl-rules", "--acl-urn", ACL_URN],
        "omitting --rule must be an error, never a silent wipe",
    );

    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

/// The empty set is a legitimate state for an ACL, so it stays expressible -
/// it just has to be asked for by name.
#[test]
fn update_acl_rules_reaches_the_empty_set_through_an_explicit_flag() {
    let command = parse_network(&["update-acl-rules", "--acl-urn", ACL_URN, "--no-rules"])
        .expect("--no-rules must parse");

    let NetworkCommands::UpdateAclRules {
        acl_urn,
        rules,
        no_rules,
    } = command
    else {
        panic!("expected update-acl-rules");
    };

    assert_eq!(acl_urn.to_string(), ACL_URN);
    assert!(rules.is_empty());
    assert!(no_rules);
}

/// The argument group must not be mistaken for "at most one `--rule`":
/// repeating the same argument is the normal way to pass a rule list.
#[test]
fn update_acl_rules_takes_a_repeated_rule() {
    let command = parse_network(&[
        "update-acl-rules",
        "--acl-urn",
        ACL_URN,
        "--rule",
        ACL_RULE,
        "--rule",
        OTHER_ACL_RULE,
    ])
    .expect("a repeated --rule must parse");

    let NetworkCommands::UpdateAclRules {
        rules, no_rules, ..
    } = command
    else {
        panic!("expected update-acl-rules");
    };

    assert_eq!(rules.len(), 2);
    assert!(!no_rules);
}

/// Asking for both at once is self-contradictory and must not silently pick
/// one - it would decide between "keep these rules" and "drop everything".
#[test]
fn update_acl_rules_refuses_a_contradictory_rule_set() {
    let err = parse_network_err(
        &[
            "update-acl-rules",
            "--acl-urn",
            ACL_URN,
            "--rule",
            ACL_RULE,
            "--no-rules",
        ],
        "--rule together with --no-rules must be an error",
    );

    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

/// `update-security-group-rules` replaces *both* lists, so the dangerous
/// invocation is stating only one of them: it used to empty the other.
#[test]
fn update_security_group_rules_refuses_to_run_without_a_stated_egress_set() {
    let err = parse_network_err(
        &[
            "update-security-group-rules",
            "--security-group-urn",
            SECURITY_GROUP_URN,
            "--ingress-rule",
            SECURITY_GROUP_RULE,
        ],
        "omitting --egress-rule must be an error, never a silent wipe",
    );

    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

#[test]
fn update_security_group_rules_refuses_to_run_without_a_stated_ingress_set() {
    let err = parse_network_err(
        &[
            "update-security-group-rules",
            "--security-group-urn",
            SECURITY_GROUP_URN,
            "--egress-rule",
            SECURITY_GROUP_RULE,
        ],
        "omitting --ingress-rule must be an error, never a silent wipe",
    );

    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

/// Each list carries its own opt-in, so "keep admitting SSH inbound, allow
/// nothing outbound" stays expressible in one invocation.
#[test]
fn update_security_group_rules_reaches_each_empty_set_independently() {
    let command = parse_network(&[
        "update-security-group-rules",
        "--security-group-urn",
        SECURITY_GROUP_URN,
        "--ingress-rule",
        SECURITY_GROUP_RULE,
        "--no-egress-rules",
    ])
    .expect("a stated ingress list plus an explicitly empty egress list must parse");

    let NetworkCommands::UpdateSecurityGroupRules {
        ingress_rules,
        no_ingress_rules,
        egress_rules,
        no_egress_rules,
        ..
    } = command
    else {
        panic!("expected update-security-group-rules");
    };

    assert_eq!(ingress_rules.len(), 1);
    assert!(!no_ingress_rules);
    assert!(egress_rules.is_empty());
    assert!(no_egress_rules);
}

/// `--relay-enabled false` with no servers is a legal, inert policy, so the
/// empty server list still has to be stated rather than implied by it.
#[test]
fn update_dns_policy_refuses_to_run_without_a_stated_upstream_server_set() {
    let err = parse_network_err(
        &[
            "update-dns-policy",
            "--dns-policy-urn",
            DNS_POLICY_URN,
            "--relay-enabled",
            "false",
        ],
        "omitting --upstream-server must be an error, never a silent wipe",
    );

    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

#[test]
fn update_dns_policy_reaches_the_empty_upstream_server_set_through_an_explicit_flag() {
    let command = parse_network(&[
        "update-dns-policy",
        "--dns-policy-urn",
        DNS_POLICY_URN,
        "--relay-enabled",
        "false",
        "--no-upstream-servers",
    ])
    .expect("--no-upstream-servers must parse");

    let NetworkCommands::UpdateDnsPolicy {
        upstream_servers,
        no_upstream_servers,
        ..
    } = command
    else {
        panic!("expected update-dns-policy");
    };

    assert!(upstream_servers.is_empty());
    assert!(no_upstream_servers);
}

/// The create is spelled like its update - a habit learned on one must not be
/// punished by the other.
#[test]
fn create_dns_policy_refuses_to_run_without_a_stated_upstream_server_set() {
    let err = parse_network_err(
        &[
            "create-dns-policy",
            "--project-urn",
            PROJECT_URN,
            "--relay-enabled",
            "false",
        ],
        "omitting --upstream-server must be an error on the create too",
    );

    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

/// `create-subnet` was the last boolean-setting
/// network command still spelled as a bare flag.
#[test]
fn create_subnet_refuses_to_run_without_an_explicit_auto_assign_public_ip_value() {
    let err = parse_network_err(
        &["create-subnet", "--vpc-urn", VPC_URN],
        "omitting --auto-assign-public-ip must be an error",
    );

    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

#[test]
fn create_subnet_takes_an_explicit_auto_assign_public_ip_value() {
    for (given, expected) in [("true", true), ("false", false)] {
        let command = parse_network(&[
            "create-subnet",
            "--vpc-urn",
            VPC_URN,
            "--auto-assign-public-ip",
            given,
        ])
        .expect("an explicit value must parse");

        let NetworkCommands::CreateSubnet {
            auto_assign_public_ip,
            ..
        } = command
        else {
            panic!("expected create-subnet");
        };

        assert_eq!(auto_assign_public_ip, expected);
    }
}

const NETWORK_INTERFACE_URN: &str =
    "urn:origin:cloud:network:eu-central1:0a1b2c3d4e5f:my-project-123:if/abc123";

/// The source-address check is resource state, so both its create
/// and its change take an explicit `true`/`false` and never a bare flag.
#[test]
fn the_source_address_check_is_never_implied() {
    for args in [
        vec![
            "create-network-interface",
            "--project-urn",
            PROJECT_URN,
            "--subnet-urn",
            SUBNET_URN,
        ],
        vec![
            "set-network-interface-source-address-check",
            "--network-interface-urn",
            NETWORK_INTERFACE_URN,
        ],
    ] {
        let err = parse_network_err(&args, "omitting --source-address-check must be an error");
        assert_eq!(
            err.kind(),
            clap::error::ErrorKind::MissingRequiredArgument,
            "{args:?}"
        );
    }

    let err = parse_network_err(
        &[
            "set-network-interface-source-address-check",
            "--network-interface-urn",
            NETWORK_INTERFACE_URN,
            "--source-address-check",
            "--network-interface-urn",
            NETWORK_INTERFACE_URN,
        ],
        "a bare --source-address-check must not parse as true",
    );
    assert_ne!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

#[test]
fn the_source_address_check_takes_an_explicit_value() {
    for (given, expected) in [("true", true), ("false", false)] {
        let NetworkCommands::SetNetworkInterfaceSourceAddressCheck {
            source_address_check,
            ..
        } = parse_network(&[
            "set-network-interface-source-address-check",
            "--network-interface-urn",
            NETWORK_INTERFACE_URN,
            "--source-address-check",
            given,
        ])
        .expect("an explicit value must parse")
        else {
            panic!("expected set-network-interface-source-address-check");
        };
        assert_eq!(source_address_check, expected);

        let NetworkCommands::CreateNetworkInterface {
            source_address_check,
            ..
        } = parse_network(&[
            "create-network-interface",
            "--project-urn",
            PROJECT_URN,
            "--subnet-urn",
            SUBNET_URN,
            "--source-address-check",
            given,
        ])
        .expect("an explicit value must parse")
        else {
            panic!("expected create-network-interface");
        };
        assert_eq!(source_address_check, expected);
    }
}

#[test]
fn setting_security_groups_takes_every_repeated_urn() {
    let other_group = SECURITY_GROUP_URN.replace("abc123", "def456");
    let NetworkCommands::SetNetworkInterfaceSecurityGroups {
        security_group_urns,
        ..
    } = parse_network(&[
        "set-network-interface-security-groups",
        "--network-interface-urn",
        NETWORK_INTERFACE_URN,
        "--security-group-urn",
        SECURITY_GROUP_URN,
        "--security-group-urn",
        &other_group,
    ])
    .expect("two groups must parse")
    else {
        panic!("expected set-network-interface-security-groups");
    };
    assert_eq!(security_group_urns.len(), 2);
}

#[test]
fn setting_security_groups_requires_at_least_one() {
    let err = parse_network_err(
        &[
            "set-network-interface-security-groups",
            "--network-interface-urn",
            NETWORK_INTERFACE_URN,
        ],
        "an empty list must be refused before it reaches the API",
    );
    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}
