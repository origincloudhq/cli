use super::rule_args::{AclRuleArg, SecurityGroupRuleArg};
use crate::confirm::Confirmation;
use crate::handler::tag_args::TagArg;
use crate::identity::Urn;
use clap::Subcommand;

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum NetworkCommands {
    /// Create an empty VPC: no subnet, route table, network ACL or security
    /// group. Create those with `create-subnet`, `create-route-table`,
    /// `create-acl` and `create-security-group`. Until `attach-dns-policy`
    /// attaches a DNS policy, the VPC relays through the project's default
    /// one.
    CreateVpc {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,

        /// IPv4 range of the VPC, in private address space (10.0.0.0/8,
        /// 172.16.0.0/12 or 192.168.0.0/16), from /16 to /28. Omitted, a
        /// random /16 from 172.16.0.0/12 that doesn't collide with your other
        /// VPCs is assigned.
        #[arg(long)]
        ipv4_cidr: Option<String>,

        /// A tag to create the VPC with, as KEY=VALUE. Repeat for more.
        /// `--tag origin:name=<name>` names the VPC.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<TagArg>,
    },

    /// Set tags on a resource. A key it already carries gets the new value;
    /// its other tags are kept. Prints all of its tags.
    SetResourceTags {
        /// URN of the resource: a VPC, subnet, network interface, security
        /// group, network ACL, Floating IP, Internet Gateway, route table or
        /// NAT gateway.
        #[arg(long)]
        resource_urn: Urn,

        /// A tag to set, as KEY=VALUE. Repeat for more.
        #[arg(long = "tag", value_name = "KEY=VALUE", required = true)]
        tags: Vec<TagArg>,
    },

    /// Remove tags from a resource by key. Prints the tags left.
    RemoveResourceTags {
        /// URN of the resource: a VPC, subnet, network interface, security
        /// group, network ACL, Floating IP, Internet Gateway, route table or
        /// NAT gateway.
        #[arg(long)]
        resource_urn: Urn,

        /// Key of a tag to remove. Repeat for more.
        #[arg(long = "key", value_name = "KEY", required = true)]
        keys: Vec<String>,
    },

    /// List the VPCs of a project.
    ListVpcs {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// Create a subnet in a VPC. In a VPC with a main route table and a
    /// default network ACL, such as a project's default VPC, the subnet gets
    /// both; otherwise it starts with neither, and its `aclUrn` and
    /// `routeTableUrn` are `null`. Until `associate-route-table` gives it a
    /// route table, it has no route to the internet. Until
    /// `update-subnet-acl` gives it a network ACL, it isn't filtered at its
    /// edge; its interfaces' security groups still apply.
    CreateSubnet {
        /// URN of the VPC.
        #[arg(long)]
        vpc_urn: Urn,

        /// Whether instances launched into the subnet automatically get a
        /// Floating IP: `true` or `false`. Required, so it reads the same as
        /// on `update-subnet-auto-assign-public-ip`.
        #[arg(long, action = clap::ArgAction::Set)]
        auto_assign_public_ip: bool,

        /// A tag to create the subnet with, as KEY=VALUE. Repeat for more.
        /// `--tag origin:name=<name>` names it.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<TagArg>,
    },

    /// List the subnets of a project. A subnet's `aclUrn` is `null` when it
    /// has no network ACL: it isn't filtered at its edge. Its
    /// `routeTableUrn` is `null` when it has no route table: it has no route
    /// to the internet.
    ListSubnets {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// Get a single Network Interface. Interfaces are created via
    /// `create-network-interface` or implicitly during instance launch, and
    /// are removed automatically on instance termination or subnet/VPC
    /// deletion - use `delete-network-interface` to remove a standalone one
    /// directly.
    GetNetworkInterface {
        /// URN of the Network Interface.
        #[arg(long)]
        network_interface_urn: Urn,
    },

    /// Delete a standalone Network Interface. Refuses while it's attached
    /// to an instance; an attached interface was created by the instance's
    /// launch and is deleted when the instance is terminated. Refuses too
    /// while a Floating IP is still associated with it -
    /// `disassociate-floating-ip` first, or the address could never be
    /// released afterwards.
    DeleteNetworkInterface {
        /// URN of the Network Interface.
        #[arg(long)]
        network_interface_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// List Network Interfaces for a project.
    ListNetworkInterfaces {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// List Floating IPs for a project.
    ListFloatingIps {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// Get a VPC's Internet Gateway, if it has one attached.
    GetInternetGateway {
        /// URN of the VPC.
        #[arg(long)]
        vpc_urn: Urn,
    },

    /// Attach an Internet Gateway to a VPC. A subnet reaches the internet
    /// through it once the subnet's route table has a default route to it:
    /// add one with `update-route-table-routes`. Only a VPC with a main
    /// route table, such as a project's default VPC, gets default routes to
    /// the gateway added to that table.
    AttachInternetGateway {
        /// URN of the VPC.
        #[arg(long)]
        vpc_urn: Urn,

        /// A tag to create the Internet Gateway with, as KEY=VALUE. Repeat for more.
        /// `--tag origin:name=<name>` names it.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<TagArg>,
    },

    /// Detach a VPC's Internet Gateway. Removes the default routes attaching
    /// it added to a main route table; refuses while any other route table
    /// still has a route to it.
    DetachInternetGateway {
        /// URN of the VPC.
        #[arg(long)]
        vpc_urn: Urn,
    },

    /// Delete a VPC and everything it owns, then release its claimed CIDR(s)
    /// back to the pool. Refuses if any subnet still has an attached
    /// network interface, or while any NAT gateway lives in the VPC - delete
    /// those with `delete-nat-gateway` first.
    DeleteVpc {
        /// URN of the VPC.
        #[arg(long)]
        vpc_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// Delete a subnet. Refuses if it still has an attached network
    /// interface, or while a NAT gateway lives in it. Never releases the
    /// owning VPC's CIDR capacity - that only happens on `delete-vpc`.
    DeleteSubnet {
        /// URN of the subnet.
        #[arg(long)]
        subnet_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// Get a Floating IP.
    GetFloatingIp {
        /// URN of the Floating IP.
        #[arg(long)]
        floating_ip_urn: Urn,
    },

    /// Allocate a Floating IP from the platform's public IPv4 pool. It stays
    /// unassociated until `associate-floating-ip`.
    AllocateFloatingIp {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,

        /// A tag to create the Floating IP with, as KEY=VALUE. Repeat for more.
        /// `--tag origin:name=<name>` names it.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<TagArg>,
    },

    /// Associate a Floating IP with a network interface, enabling static 1:1
    /// NAT for it. Errors if already associated - `disassociate-floating-ip`
    /// first.
    AssociateFloatingIp {
        /// URN of the Floating IP.
        #[arg(long)]
        floating_ip_urn: Urn,

        /// URN of the network interface to associate it with.
        #[arg(long)]
        network_interface_urn: Urn,
    },

    /// Disassociate a Floating IP, removing its NAT rules. Idempotent.
    DisassociateFloatingIp {
        /// URN of the Floating IP.
        #[arg(long)]
        floating_ip_urn: Urn,
    },

    /// Release a Floating IP back to the pool and delete it. Errors if still
    /// associated - `disassociate-floating-ip` first.
    ReleaseFloatingIp {
        /// URN of the Floating IP.
        #[arg(long)]
        floating_ip_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// Get a single VPC.
    GetVpc {
        /// URN of the VPC.
        #[arg(long)]
        vpc_urn: Urn,
    },

    /// Get a single Subnet. Its `aclUrn` is `null` when it has no network
    /// ACL: it isn't filtered at its edge, and its interfaces' security
    /// groups are what applies. Its `routeTableUrn` is `null` when it has no
    /// route table: it has no route out of its VPC, so no route to the
    /// internet.
    GetSubnet {
        /// URN of the subnet.
        #[arg(long)]
        subnet_urn: Urn,
    },

    /// Get a Security Group and its rules.
    GetSecurityGroup {
        /// URN of the Security Group.
        #[arg(long)]
        security_group_urn: Urn,
    },

    /// List every Security Group in a project.
    ListSecurityGroups {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// Create a Security Group in a VPC. Starts with no ingress/egress
    /// rules - use `update-security-group-rules` afterwards to add rules.
    CreateSecurityGroup {
        /// URN of the VPC.
        #[arg(long)]
        vpc_urn: Urn,

        /// A tag to create the security group with, as KEY=VALUE. Repeat for more.
        /// `--tag origin:name=<name>` names it.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<TagArg>,
    },

    /// Delete a non-default Security Group. Refuses to delete a VPC's
    /// default Security Group, or one a Network Interface still carries:
    /// remove it with `set-network-interface-security-groups` first.
    DeleteSecurityGroup {
        /// URN of the Security Group.
        #[arg(long)]
        security_group_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// Get a Network ACL and its rules.
    GetAcl {
        /// URN of the Network ACL.
        #[arg(long)]
        acl_urn: Urn,
    },

    /// List every Network ACL in a project.
    ListAcls {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// List the NAT Gateways in a project.
    ListNatGateways {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// Create a NAT Gateway so Instances without a Floating IP can reach
    /// the internet.
    ///
    /// It goes in a **public** Subnet - one whose Route Table has a default
    /// route to an Internet Gateway - and uses a Floating IP you have
    /// already allocated and not attached to anything. Point a private
    /// Subnet's Route Table at it with `update-route-table-routes` to send
    /// that Subnet's traffic through it.
    ///
    /// Instances behind it reach the internet as the Gateway's address and
    /// are not reachable from it. The Floating IP stays yours when the
    /// Gateway is deleted.
    CreateNatGateway {
        /// URN of the public Subnet.
        #[arg(long)]
        subnet_urn: Urn,

        /// URN of an allocated, unattached Floating IP.
        #[arg(long)]
        floating_ip_urn: Urn,

        /// A tag to create the NAT gateway with, as KEY=VALUE. Repeat for more.
        /// `--tag origin:name=<name>` names it.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<TagArg>,
    },

    /// Delete a NAT Gateway. Refuses while any Route Table still routes
    /// through it.
    DeleteNatGateway {
        /// URN of the NAT Gateway.
        #[arg(long)]
        nat_gateway_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// List the Route Tables in a project.
    ListRouteTables {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// Show one Route Table.
    GetRouteTable {
        /// URN of the Route Table.
        #[arg(long)]
        route_table_urn: Urn,
    },

    /// Create a Route Table in a VPC.
    ///
    /// It starts with the VPC's local routes and no default route, so a
    /// Subnet associated with it reaches its own VPC and nothing else -
    /// the "private subnet" half of the public/private pattern. Add a
    /// default with `update-route-table-routes`, and associate it with a
    /// Subnet with `associate-route-table`.
    CreateRouteTable {
        /// URN of the VPC.
        #[arg(long)]
        vpc_urn: Urn,

        /// A tag to create the route table with, as KEY=VALUE. Repeat for more.
        /// `--tag origin:name=<name>` names it.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<TagArg>,
    },

    /// Replace a Route Table's routes.
    ///
    /// This is a wholesale replace, not a patch: pass every route the
    /// table should have. `--route` is repeatable and takes
    /// `<destination>,<target>[,<target-urn>]`, where `<target>` is
    /// `local`, `internet-gateway` or `nat-gateway` - e.g.
    /// `--route '0.0.0.0/0,internet-gateway,ffs:network:...'`,
    /// `--route '0.0.0.0/0,nat-gateway,ffs:network:...'` or
    /// `--route '10.0.0.0/16,local'`. A `nat-gateway` route carries a
    /// subnet's whole way out, so its destination must be `0.0.0.0/0`:
    /// it translates IPv4 only.
    UpdateRouteTableRoutes {
        /// URN of the Route Table.
        #[arg(long)]
        route_table_urn: Urn,

        /// Repeatable. `<destination>,<target>[,<target-urn>]`.
        #[arg(long = "route", required = true)]
        routes: Vec<String>,
    },

    /// Delete a Route Table. Refuses a VPC's main table, and one a Subnet
    /// is still associated with.
    DeleteRouteTable {
        /// URN of the Route Table.
        #[arg(long)]
        route_table_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// Point a Subnet at a Route Table, replacing whichever it used before.
    /// A Subnet with no Route Table has no route out of its VPC.
    AssociateRouteTable {
        /// URN of the Subnet.
        #[arg(long)]
        subnet_urn: Urn,

        /// URN of the Route Table.
        #[arg(long)]
        route_table_urn: Urn,
    },

    /// Create a Network ACL in a VPC, for filtering traffic at the edge of
    /// the subnets it is assigned to. Starts with no rules - use
    /// `update-acl-rules` to add rules, and `update-subnet-acl` to assign it
    /// to a subnet.
    CreateAcl {
        /// URN of the VPC.
        #[arg(long)]
        vpc_urn: Urn,

        /// A tag to create the network ACL with, as KEY=VALUE. Repeat for more.
        /// `--tag origin:name=<name>` names it.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<TagArg>,
    },

    /// Delete a non-default Network ACL. Refuses to delete a VPC's default
    /// ACL, or one still assigned to a Subnet.
    DeleteAcl {
        /// URN of the Network ACL.
        #[arg(long)]
        acl_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// Set which Network ACL enforces a subnet's traffic. A subnet with no
    /// ACL isn't filtered at its edge; its interfaces' security groups still
    /// apply. Refuses if the ACL belongs to a different VPC than the subnet.
    UpdateSubnetAcl {
        /// URN of the subnet.
        #[arg(long)]
        subnet_urn: Urn,
        /// URN of the Network ACL to assign.
        #[arg(long)]
        acl_urn: Urn,
    },

    /// Change whether instances launched into a subnet automatically get a
    /// Floating IP. Applies only to network interfaces created after the
    /// change - existing ones keep whatever Floating IP state they already
    /// have (use `associate-floating-ip`/`disassociate-floating-ip` for
    /// those).
    UpdateSubnetAutoAssignPublicIp {
        /// URN of the subnet.
        #[arg(long)]
        subnet_urn: Urn,

        /// The new value. Required and explicit (`true`/`false`) rather
        /// than a bare flag, so that omitting it can never silently mean
        /// "disable".
        #[arg(long, action = clap::ArgAction::Set)]
        auto_assign_public_ip: bool,
    },

    /// Get a DNS policy.
    GetDnsPolicy {
        /// URN of the DNS policy.
        #[arg(long)]
        dns_policy_urn: Urn,
    },

    /// List every DNS relay policy in a project.
    ListDnsPolicies {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// Create an additional, non-default DNS policy in a project. It takes
    /// the same options as `update-dns-policy`.
    #[command(group = clap::ArgGroup::new("create_upstream_server_set").required(true).multiple(false).args(["upstream_servers", "no_upstream_servers"]))]
    CreateDnsPolicy {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,

        /// Whether this policy relays unanswered queries upstream at all.
        /// Required and explicit (`true`/`false`) rather than a bare flag,
        /// so that it reads the same way as on `update-dns-policy`, where
        /// omitting it could otherwise silently mean "disable".
        #[arg(long, action = clap::ArgAction::Set)]
        relay_enabled: bool,

        /// IPv4/IPv6 address of an upstream DNS server, repeatable.
        /// Required non-empty only while `--relay-enabled true` is given.
        #[arg(long = "upstream-server")]
        upstream_servers: Vec<String>,

        /// Start the policy with no upstream servers - only legal
        /// alongside `--relay-enabled false`. Exactly one of
        /// `--upstream-server` and `--no-upstream-servers` must be given.
        #[arg(long)]
        no_upstream_servers: bool,
    },

    /// Replace a DNS relay policy's own settings wholesale - not
    /// which policy is default or which VPC(s) it backs. The upstream
    /// server list is replaced on every invocation, so its new contents
    /// must be stated: `--upstream-server` for a non-empty list,
    /// `--no-upstream-servers` for an empty one.
    #[command(group = clap::ArgGroup::new("update_upstream_server_set").required(true).multiple(false).args(["upstream_servers", "no_upstream_servers"]))]
    UpdateDnsPolicy {
        /// URN of the DNS policy.
        #[arg(long)]
        dns_policy_urn: Urn,

        /// Whether this policy relays unanswered queries upstream at all.
        /// Required and explicit (`true`/`false`) rather than a bare flag,
        /// so that omitting it can never silently mean "disable" on a
        /// policy that currently relays.
        #[arg(long, action = clap::ArgAction::Set)]
        relay_enabled: bool,

        /// IPv4/IPv6 address of an upstream DNS server, repeatable - the
        /// full given set replaces the existing upstream server list.
        /// Required non-empty while `--relay-enabled true` is given.
        #[arg(long = "upstream-server")]
        upstream_servers: Vec<String>,

        /// Replace the upstream server list with the empty set - only
        /// legal alongside `--relay-enabled false`. Exactly one of
        /// `--upstream-server` and `--no-upstream-servers` must be given.
        #[arg(long)]
        no_upstream_servers: bool,
    },

    /// Reassign which policy is a project's default DNS relay policy. A VPC
    /// attached to a policy keeps it; a VPC attached to none relays through
    /// the project's default and follows this change.
    SetDefaultDnsPolicy {
        /// URN of the DNS policy to make default.
        #[arg(long)]
        dns_policy_urn: Urn,
    },

    /// Delete a DNS relay policy. Refuses if it's the project's
    /// default, or if any VPC is still attached to it.
    DeleteDnsPolicy {
        /// URN of the DNS policy.
        #[arg(long)]
        dns_policy_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// Attach a VPC to a DNS relay policy, replacing the one it was attached
    /// to. A VPC attached to none relays through the project's default
    /// policy.
    AttachDnsPolicy {
        /// URN of the VPC.
        #[arg(long)]
        vpc_urn: Urn,

        /// URN of the DNS policy to attach - must belong to the VPC's own
        /// project.
        #[arg(long)]
        dns_policy_urn: Urn,
    },

    /// Create a standalone network interface in a subnet.
    CreateNetworkInterface {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,

        /// URN of the subnet.
        #[arg(long)]
        subnet_urn: Urn,

        /// URN of a Security Group to attach. Repeatable. Omitted, the
        /// interface gets the owning VPC's default Security Group when the
        /// VPC has one, and none otherwise: an interface in no Security
        /// Group admits and sends nothing but ARP, IPv6 neighbor discovery
        /// and replies to its own traffic.
        #[arg(long = "security-group-urn")]
        security_group_urns: Vec<Urn>,

        /// Whether the source-address check applies to the interface:
        /// `true` or `false`. Required, so it reads the same as on
        /// `set-network-interface-source-address-check`.
        #[arg(long, action = clap::ArgAction::Set)]
        source_address_check: bool,

        /// A tag to create the network interface with, as KEY=VALUE. Repeat for more.
        /// `--tag origin:name=<name>` names it.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<TagArg>,
    },

    /// Turn the source-address check on or off for a network interface.
    /// The check drops anything the interface sends from a MAC or IP
    /// address that is not its own; turn it off for a router or NAT
    /// instance, a Kubernetes node routing pod traffic, or a holder of a
    /// floating virtual IP; with it off, the interface can send from any
    /// address. A change reaches a running instance without a stop and
    /// start.
    SetNetworkInterfaceSourceAddressCheck {
        /// URN of the Network Interface.
        #[arg(long)]
        network_interface_urn: Urn,

        /// `true` to apply the check, `false` to let the interface send
        /// from any address. Required and explicit rather than a bare flag.
        #[arg(long, action = clap::ArgAction::Set)]
        source_address_check: bool,
    },

    /// Replace the Security Groups a Network Interface carries. State the
    /// whole new list with `--security-group-urn` (repeatable): a group left
    /// out is removed. Every group must be in the interface's VPC. A change
    /// reaches a running instance without a stop and start.
    SetNetworkInterfaceSecurityGroups {
        /// URN of the Network Interface.
        #[arg(long)]
        network_interface_urn: Urn,

        /// URN of a Security Group the interface carries from now on.
        /// Repeatable; at least one.
        #[arg(long = "security-group-urn", required = true)]
        security_group_urns: Vec<Urn>,
    },

    /// Replace a Network ACL's full rule list. State the new list with
    /// `--rule` (repeatable), or `--no-rules` for the empty one; stating
    /// neither is an error rather than a silent wipe.
    #[command(group = clap::ArgGroup::new("acl_rule_set").required(true).multiple(false).args(["rules", "no_rules"]))]
    UpdateAclRules {
        /// URN of the Network ACL.
        #[arg(long)]
        acl_urn: Urn,

        /// One ACL rule, repeatable - the full given set replaces the
        /// existing rules. Format:
        /// `priority=<u16>,direction=<ingress|egress>,protocol=<any|tcp|udp|icmp>,port=<N|N-M|any>,source=<cidr>,policy=<accept|drop>`.
        /// `icmp` covers ICMP and ICMPv6 and takes `port=any`.
        #[arg(long = "rule")]
        rules: Vec<AclRuleArg>,

        /// Replace the rules with the empty set, leaving the ACL
        /// deny-by-default. Required to say this out loud: exactly one of
        /// `--rule` and `--no-rules` must be given.
        #[arg(long)]
        no_rules: bool,
    },

    /// Replace a Security Group's full ingress and egress rule lists. Both
    /// lists are replaced on every invocation, so each one's new contents
    /// must be stated - `--ingress-rule`/`--egress-rule` for a non-empty
    /// list, `--no-ingress-rules`/`--no-egress-rules` for an empty one.
    /// Stating neither for a list is an error rather than a silent wipe
    ///.
    #[command(group = clap::ArgGroup::new("ingress_rule_set").required(true).multiple(false).args(["ingress_rules", "no_ingress_rules"]))]
    #[command(group = clap::ArgGroup::new("egress_rule_set").required(true).multiple(false).args(["egress_rules", "no_egress_rules"]))]
    UpdateSecurityGroupRules {
        /// URN of the Security Group.
        #[arg(long)]
        security_group_urn: Urn,

        /// One ingress rule, repeatable - the full given set replaces the
        /// existing ingress rules. Format:
        /// `protocol=<any|tcp|udp|icmp>,port=<N|N-M|any>,(ip-range=<cidr>|security-group-urn=<urn>)[,description=<text>]`.
        /// `icmp` covers ICMP and ICMPv6 and takes `port=any`.
        #[arg(long = "ingress-rule")]
        ingress_rules: Vec<SecurityGroupRuleArg>,

        /// Replace the ingress rules with the empty set, admitting no
        /// inbound traffic. Exactly one of `--ingress-rule` and
        /// `--no-ingress-rules` must be given.
        #[arg(long)]
        no_ingress_rules: bool,

        /// One egress rule, repeatable - same format as `--ingress-rule`.
        /// The full given set replaces the existing egress rules.
        #[arg(long = "egress-rule")]
        egress_rules: Vec<SecurityGroupRuleArg>,

        /// Replace the egress rules with the empty set, admitting no
        /// outbound traffic. Exactly one of `--egress-rule` and
        /// `--no-egress-rules` must be given.
        #[arg(long)]
        no_egress_rules: bool,
    },
}
