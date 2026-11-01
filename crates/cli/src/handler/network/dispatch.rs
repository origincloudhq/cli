use super::commands::NetworkCommands;
use crate::config::ClientProfile;
use crate::output::SerializableOutput;
use crate::service_apis::network::NetworkServiceClient;
use url::Url;

pub async fn run_network_command(
    _profile: &ClientProfile,
    api_url: &Url,
    command: NetworkCommands,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let client = NetworkServiceClient::new(api_url.clone());

    match command {
        NetworkCommands::CreateVpc {
            project_urn,
            ipv4_cidr,
            tags,
        } => Ok(Box::new(
            client.create_vpc(project_urn, ipv4_cidr, tags).await?,
        )),
        NetworkCommands::SetResourceTags { resource_urn, tags } => Ok(Box::new(
            client.set_resource_tags(resource_urn, tags).await?,
        )),
        NetworkCommands::RemoveResourceTags { resource_urn, keys } => Ok(Box::new(
            client.remove_resource_tags(resource_urn, keys).await?,
        )),
        NetworkCommands::ListVpcs { project_urn } => {
            Ok(Box::new(client.list_vpcs(project_urn).await?))
        }
        NetworkCommands::CreateSubnet {
            vpc_urn,
            auto_assign_public_ip,
            tags,
        } => Ok(Box::new(
            client
                .create_subnet(vpc_urn, auto_assign_public_ip, tags)
                .await?,
        )),
        NetworkCommands::ListSubnets { project_urn } => {
            Ok(Box::new(client.list_subnets(project_urn).await?))
        }
        NetworkCommands::GetNetworkInterface {
            network_interface_urn,
        } => Ok(Box::new(
            client.get_network_interface(network_interface_urn).await?,
        )),
        NetworkCommands::DeleteNetworkInterface {
            network_interface_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_network_interface(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "network interface",
                            target: &network_interface_urn.to_string(),
                            consequence: "It can't be restored.",
                        },
                    )?,
                    network_interface_urn,
                )
                .await?,
        )),
        NetworkCommands::ListNetworkInterfaces { project_urn } => {
            Ok(Box::new(client.list_network_interfaces(project_urn).await?))
        }
        NetworkCommands::ListFloatingIps { project_urn } => {
            Ok(Box::new(client.list_floating_ips(project_urn).await?))
        }
        NetworkCommands::GetInternetGateway { vpc_urn } => {
            Ok(Box::new(client.get_internet_gateway(vpc_urn).await?))
        }
        NetworkCommands::AttachInternetGateway { vpc_urn, tags } => Ok(Box::new(
            client.attach_internet_gateway(vpc_urn, tags).await?,
        )),
        NetworkCommands::DetachInternetGateway { vpc_urn } => {
            Ok(Box::new(client.detach_internet_gateway(vpc_urn).await?))
        }
        NetworkCommands::DeleteVpc {
            vpc_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_vpc(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "VPC",
                            target: &vpc_urn.to_string(),
                            consequence: "It can't be restored.",
                        },
                    )?,
                    vpc_urn,
                )
                .await?,
        )),
        NetworkCommands::DeleteSubnet {
            subnet_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_subnet(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "subnet",
                            target: &subnet_urn.to_string(),
                            consequence: "It can't be restored.",
                        },
                    )?,
                    subnet_urn,
                )
                .await?,
        )),
        NetworkCommands::GetFloatingIp { floating_ip_urn } => {
            Ok(Box::new(client.get_floating_ip(floating_ip_urn).await?))
        }
        NetworkCommands::AllocateFloatingIp { project_urn, tags } => Ok(Box::new(
            client.allocate_floating_ip(project_urn, tags).await?,
        )),
        NetworkCommands::AssociateFloatingIp {
            floating_ip_urn,
            network_interface_urn,
        } => Ok(Box::new(
            client
                .associate_floating_ip(floating_ip_urn, network_interface_urn)
                .await?,
        )),
        NetworkCommands::DisassociateFloatingIp { floating_ip_urn } => Ok(Box::new(
            client.disassociate_floating_ip(floating_ip_urn).await?,
        )),
        NetworkCommands::ReleaseFloatingIp {
            floating_ip_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .release_floating_ip(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "release",
                            what: "Floating IP",
                            target: &floating_ip_urn.to_string(),
                            consequence:
                                "The address goes back to the pool; you may not get it again.",
                        },
                    )?,
                    floating_ip_urn,
                )
                .await?,
        )),
        NetworkCommands::GetVpc { vpc_urn } => Ok(Box::new(client.get_vpc(vpc_urn).await?)),
        NetworkCommands::GetSubnet { subnet_urn } => {
            Ok(Box::new(client.get_subnet(subnet_urn).await?))
        }
        NetworkCommands::GetSecurityGroup { security_group_urn } => Ok(Box::new(
            client.get_security_group(security_group_urn).await?,
        )),
        NetworkCommands::ListSecurityGroups { project_urn } => {
            Ok(Box::new(client.list_security_groups(project_urn).await?))
        }
        NetworkCommands::CreateSecurityGroup { vpc_urn, tags } => {
            Ok(Box::new(client.create_security_group(vpc_urn, tags).await?))
        }
        NetworkCommands::DeleteSecurityGroup {
            security_group_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_security_group(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "security group",
                            target: &security_group_urn.to_string(),
                            consequence: "Its rules are lost.",
                        },
                    )?,
                    security_group_urn,
                )
                .await?,
        )),
        NetworkCommands::GetAcl { acl_urn } => Ok(Box::new(client.get_acl(acl_urn).await?)),
        NetworkCommands::ListAcls { project_urn } => {
            Ok(Box::new(client.list_acls(project_urn).await?))
        }
        NetworkCommands::ListNatGateways { project_urn } => {
            Ok(Box::new(client.list_nat_gateways(project_urn).await?))
        }
        NetworkCommands::CreateNatGateway {
            subnet_urn,
            floating_ip_urn,
            tags,
        } => Ok(Box::new(
            client
                .create_nat_gateway(subnet_urn, floating_ip_urn, tags)
                .await?,
        )),
        NetworkCommands::DeleteNatGateway {
            nat_gateway_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_nat_gateway(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "NAT gateway",
                            target: &nat_gateway_urn.to_string(),
                            consequence: "It can't be restored.",
                        },
                    )?,
                    nat_gateway_urn,
                )
                .await?,
        )),
        NetworkCommands::ListRouteTables { project_urn } => {
            Ok(Box::new(client.list_route_tables(project_urn).await?))
        }
        NetworkCommands::GetRouteTable { route_table_urn } => {
            Ok(Box::new(client.get_route_table(route_table_urn).await?))
        }
        NetworkCommands::CreateRouteTable { vpc_urn, tags } => {
            Ok(Box::new(client.create_route_table(vpc_urn, tags).await?))
        }
        NetworkCommands::UpdateRouteTableRoutes {
            route_table_urn,
            routes,
        } => {
            let parsed = routes
                .iter()
                .map(|route| parse_route(route))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(Box::new(
                client
                    .update_route_table_routes(route_table_urn, parsed)
                    .await?,
            ))
        }
        NetworkCommands::DeleteRouteTable {
            route_table_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_route_table(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "route table",
                            target: &route_table_urn.to_string(),
                            consequence: "Its routes are lost.",
                        },
                    )?,
                    route_table_urn,
                )
                .await?,
        )),
        NetworkCommands::AssociateRouteTable {
            subnet_urn,
            route_table_urn,
        } => Ok(Box::new(
            client
                .associate_route_table(subnet_urn, route_table_urn)
                .await?,
        )),
        NetworkCommands::CreateAcl { vpc_urn, tags } => {
            Ok(Box::new(client.create_acl(vpc_urn, tags).await?))
        }
        NetworkCommands::DeleteAcl {
            acl_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_acl(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "network ACL",
                            target: &acl_urn.to_string(),
                            consequence: "Its rules are lost.",
                        },
                    )?,
                    acl_urn,
                )
                .await?,
        )),
        NetworkCommands::UpdateSubnetAcl {
            subnet_urn,
            acl_urn,
        } => Ok(Box::new(
            client.update_subnet_acl(subnet_urn, acl_urn).await?,
        )),
        NetworkCommands::UpdateSubnetAutoAssignPublicIp {
            subnet_urn,
            auto_assign_public_ip,
        } => Ok(Box::new(
            client
                .update_subnet_auto_assign_public_ip(subnet_urn, auto_assign_public_ip)
                .await?,
        )),
        NetworkCommands::GetDnsPolicy { dns_policy_urn } => {
            Ok(Box::new(client.get_dns_policy(dns_policy_urn).await?))
        }
        NetworkCommands::ListDnsPolicies { project_urn } => {
            Ok(Box::new(client.list_dns_policies(project_urn).await?))
        }
        NetworkCommands::CreateDnsPolicy {
            project_urn,
            relay_enabled,
            upstream_servers,
            no_upstream_servers,
        } => {
            debug_assert_empty_set_was_asked_for(&upstream_servers, no_upstream_servers);
            Ok(Box::new(
                client
                    .create_dns_policy(project_urn, relay_enabled, upstream_servers)
                    .await?,
            ))
        }
        NetworkCommands::UpdateDnsPolicy {
            dns_policy_urn,
            relay_enabled,
            upstream_servers,
            no_upstream_servers,
        } => {
            debug_assert_empty_set_was_asked_for(&upstream_servers, no_upstream_servers);
            Ok(Box::new(
                client
                    .update_dns_policy(dns_policy_urn, relay_enabled, upstream_servers)
                    .await?,
            ))
        }
        NetworkCommands::SetDefaultDnsPolicy { dns_policy_urn } => Ok(Box::new(
            client.set_default_dns_policy(dns_policy_urn).await?,
        )),
        NetworkCommands::DeleteDnsPolicy {
            dns_policy_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_dns_policy(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "DNS policy",
                            target: &dns_policy_urn.to_string(),
                            consequence: "Its settings are lost.",
                        },
                    )?,
                    dns_policy_urn,
                )
                .await?,
        )),
        NetworkCommands::AttachDnsPolicy {
            vpc_urn,
            dns_policy_urn,
        } => Ok(Box::new(
            client.attach_dns_policy(vpc_urn, dns_policy_urn).await?,
        )),
        NetworkCommands::CreateNetworkInterface {
            project_urn,
            subnet_urn,
            security_group_urns,
            source_address_check,
            tags,
        } => Ok(Box::new(
            client
                .create_network_interface(
                    project_urn,
                    subnet_urn,
                    security_group_urns,
                    source_address_check,
                    tags,
                )
                .await?,
        )),
        NetworkCommands::SetNetworkInterfaceSourceAddressCheck {
            network_interface_urn,
            source_address_check,
        } => Ok(Box::new(
            client
                .set_network_interface_source_address_check(
                    network_interface_urn,
                    source_address_check,
                )
                .await?,
        )),
        NetworkCommands::SetNetworkInterfaceSecurityGroups {
            network_interface_urn,
            security_group_urns,
        } => Ok(Box::new(
            client
                .set_network_interface_security_groups(network_interface_urn, security_group_urns)
                .await?,
        )),
        NetworkCommands::UpdateAclRules {
            acl_urn,
            rules,
            no_rules,
        } => {
            debug_assert_empty_set_was_asked_for(&rules, no_rules);
            Ok(Box::new(client.update_acl_rules(acl_urn, rules).await?))
        }
        NetworkCommands::UpdateSecurityGroupRules {
            security_group_urn,
            ingress_rules,
            no_ingress_rules,
            egress_rules,
            no_egress_rules,
        } => {
            debug_assert_empty_set_was_asked_for(&ingress_rules, no_ingress_rules);
            debug_assert_empty_set_was_asked_for(&egress_rules, no_egress_rules);
            Ok(Box::new(
                client
                    .update_security_group_rules(security_group_urn, ingress_rules, egress_rules)
                    .await?,
            ))
        }
    }
}

/// Every wholesale-replace update in `commands.rs` pairs its repeatable list
/// argument with a `--no-<list>` flag inside a required, mutually-exclusive clap
/// argument group, so that "replace with the empty set" has to be asked for
/// by name and omitting both fails at parse time.
///
/// The group makes the two exactly equivalent - a non-empty list iff the
/// flag is absent - which is what the handler relies on when it forwards
/// the (already empty) list unchanged. Pinned here so that a later edit
/// dropping or loosening a group is caught by the parse-level tests rather
/// than by an emptied ACL in production.
fn debug_assert_empty_set_was_asked_for<T>(items: &[T], empty_requested: bool) {
    debug_assert_eq!(
        items.is_empty(),
        empty_requested,
        "clap's required argument group must guarantee exactly one of the list and its --no-* flag"
    );
}

/// Parses a `--route` value: `<destination>,<target>[,<target-urn>]`.
///
/// A positional comma-separated form rather than a repeated flag group,
/// because clap cannot express "these three flags repeat together" and the
/// alternative — one `--route-destination` per `--route-target` matched by
/// position — is the shape that silently pairs the wrong two when someone
/// miscounts.
fn parse_route(
    spec: &str,
) -> anyhow::Result<
    crate::service_apis::network::update_route_table_routes::NetworkRouteTableRouteInput,
> {
    use crate::service_apis::network::update_route_table_routes::{
        NetworkRouteTableRouteInput, NetworkRouteTarget,
    };

    let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
    let (destination, target) = match parts.as_slice() {
        [destination, target] | [destination, target, _] => (*destination, *target),
        _ => anyhow::bail!(
            "route {spec:?} must be '<destination>,<target>[,<target-urn>]', e.g. \
             '0.0.0.0/0,internet-gateway,ffs:network:...'"
        ),
    };
    let target_urn = parts.get(2).copied().filter(|urn| !urn.is_empty());

    let (target, target_urn) = match (target.to_ascii_lowercase().as_str(), target_urn) {
        ("local", None) => (NetworkRouteTarget::LOCAL, None),
        ("local", Some(_)) => anyhow::bail!("a local route names no target urn"),
        ("internet-gateway" | "internet_gateway", Some(urn)) => (
            NetworkRouteTarget::INTERNET_GATEWAY,
            Some(urn.parse().map_err(|err| {
                anyhow::anyhow!("route {spec:?} has an invalid target urn: {err}")
            })?),
        ),
        ("internet-gateway" | "internet_gateway", None) => {
            anyhow::bail!("an internet-gateway route needs a target urn as the third field")
        }
        ("nat-gateway" | "nat_gateway", Some(urn)) => (
            NetworkRouteTarget::NAT_GATEWAY,
            Some(urn.parse().map_err(|err| {
                anyhow::anyhow!("route {spec:?} has an invalid target urn: {err}")
            })?),
        ),
        ("nat-gateway" | "nat_gateway", None) => {
            anyhow::bail!("a nat-gateway route needs a target urn as the third field")
        }
        (other, _) => anyhow::bail!(
            "route {spec:?} has unknown target {other:?} - expected 'local', 'internet-gateway' \
             or 'nat-gateway'"
        ),
    };

    Ok(NetworkRouteTableRouteInput {
        destination: destination.to_string(),
        target,
        target_urn,
    })
}

#[cfg(test)]
mod parse_route_tests {
    use super::parse_route;
    use crate::service_apis::network::update_route_table_routes::NetworkRouteTarget;

    const NAT_URN: &str = "urn:origin:cloud:network::0a1b2c3d4e5f:my-project-123:nat-9pFaVHsPH5z";
    const IGW_URN: &str = "urn:origin:cloud:network::0a1b2c3d4e5f:my-project-123:igw-4Azqsvv6eiU";

    /// The gap this module exists for: `createNatGateway` shipped a working
    /// resource, and the one command that points a subnet at it did not
    /// know the word, so the documented three-step flow stopped at step
    /// three with "unknown target".
    #[test]
    fn a_nat_gateway_route_parses_with_its_urn() {
        let route = parse_route(&format!("0.0.0.0/0,nat-gateway,{NAT_URN}")).expect("should parse");

        assert_eq!(route.destination, "0.0.0.0/0");
        assert!(matches!(route.target, NetworkRouteTarget::NAT_GATEWAY));
        assert!(route.target_urn.is_some());
    }

    /// Underscores accepted beside dashes, the same as the internet
    /// gateway's spelling.
    #[test]
    fn the_underscore_spelling_parses_too() {
        let route = parse_route(&format!("0.0.0.0/0,nat_gateway,{NAT_URN}")).expect("should parse");

        assert!(matches!(route.target, NetworkRouteTarget::NAT_GATEWAY));
    }

    /// Both gateway targets name a resource; the server refuses a route
    /// that does not, so refusing here reports it where the customer can
    /// still fix it.
    #[test]
    fn a_nat_gateway_route_without_a_urn_is_refused_by_name() {
        let err = parse_route("0.0.0.0/0,nat-gateway")
            .err()
            .expect("a gateway route needs its urn");

        assert!(err.to_string().contains("nat-gateway"), "{err}");
        assert!(err.to_string().contains("target urn"), "{err}");
    }

    #[test]
    fn an_unknown_target_lists_every_spelling_that_works() {
        let err = parse_route("0.0.0.0/0,peering,urn:x")
            .err()
            .expect("unknown target");
        let message = err.to_string();

        for expected in ["local", "internet-gateway", "nat-gateway"] {
            assert!(
                message.contains(expected),
                "{expected} missing from: {message}"
            );
        }
    }

    /// The targets that already worked keep working - this change adds an
    /// arm, it does not reshape the match.
    #[test]
    fn local_and_internet_gateway_routes_are_unchanged() {
        let local = parse_route("172.30.0.0/16,local").expect("local should parse");
        assert!(matches!(local.target, NetworkRouteTarget::LOCAL));
        assert!(local.target_urn.is_none());

        let igw = parse_route(&format!("0.0.0.0/0,internet-gateway,{IGW_URN}"))
            .expect("igw should parse");
        assert!(matches!(igw.target, NetworkRouteTarget::INTERNET_GATEWAY));
        assert!(igw.target_urn.is_some());
    }
}
