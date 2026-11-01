use crate::client_auth::ConfigAuthenticator;
use crate::graphql::Error as ClientError;
use crate::graphql::{GenericGraphQLClient, Retries};
use crate::handler::network::rule_args::{
    AclDirectionArg, AclPolicyArg, AclRuleArg, ProtocolArg, SecurityGroupRuleArg,
};
use crate::handler::tag_args::{tag_inputs, TagArg};
#[allow(unused_imports)]
use crate::identity::*;
use crate::output::{SerializableOutput, SerializationError};
use graphql_client::*;
use url::Url;

pub type DateTime = chrono::DateTime<chrono::Utc>;
pub type Result<T> = std::result::Result<T, ClientError>;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_create_vpc.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CreateVpc;

#[derive(serde::Serialize)]
pub(crate) struct CreateVpcResponse(create_vpc::ResponseData);

impl SerializableOutput for CreateVpcResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_list_vpcs.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListVpcs;

#[derive(serde::Serialize)]
pub(crate) struct ListVpcsResponse(list_vpcs::ResponseData);

impl SerializableOutput for ListVpcsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_create_subnet.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CreateSubnet;

#[derive(serde::Serialize)]
pub(crate) struct CreateSubnetResponse(crate::service_apis::network::create_subnet::ResponseData);

impl SerializableOutput for crate::service_apis::network::CreateSubnetResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_list_subnets.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListSubnets;

#[derive(serde::Serialize)]
pub(crate) struct ListSubnetsResponse(crate::service_apis::network::list_subnets::ResponseData);

impl SerializableOutput for crate::service_apis::network::ListSubnetsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_get_network_interface.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetNetworkInterface;

#[derive(serde::Serialize)]
pub(crate) struct GetNetworkInterfaceResponse(
    crate::service_apis::network::get_network_interface::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::GetNetworkInterfaceResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_list_network_interfaces.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListNetworkInterfaces;

#[derive(serde::Serialize)]
pub(crate) struct ListNetworkInterfacesResponse(
    crate::service_apis::network::list_network_interfaces::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::ListNetworkInterfacesResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_delete_network_interface.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DeleteNetworkInterface;

#[derive(serde::Serialize)]
pub(crate) struct DeleteNetworkInterfaceResponse(
    crate::service_apis::network::delete_network_interface::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::DeleteNetworkInterfaceResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_list_floating_ips.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListFloatingIps;

#[derive(serde::Serialize)]
pub(crate) struct ListFloatingIpsResponse(
    crate::service_apis::network::list_floating_ips::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::ListFloatingIpsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_get_internet_gateway.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetInternetGateway;

#[derive(serde::Serialize)]
pub(crate) struct GetInternetGatewayResponse(
    crate::service_apis::network::get_internet_gateway::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::GetInternetGatewayResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_attach_internet_gateway.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct AttachInternetGateway;

#[derive(serde::Serialize)]
pub(crate) struct AttachInternetGatewayResponse(
    crate::service_apis::network::attach_internet_gateway::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::AttachInternetGatewayResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_detach_internet_gateway.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DetachInternetGateway;

#[derive(serde::Serialize)]
pub(crate) struct DetachInternetGatewayResponse(
    crate::service_apis::network::detach_internet_gateway::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::DetachInternetGatewayResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_delete_vpc.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DeleteVpc;

#[derive(serde::Serialize)]
pub(crate) struct DeleteVpcResponse(crate::service_apis::network::delete_vpc::ResponseData);

impl SerializableOutput for crate::service_apis::network::DeleteVpcResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_delete_subnet.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DeleteSubnet;

#[derive(serde::Serialize)]
pub(crate) struct DeleteSubnetResponse(crate::service_apis::network::delete_subnet::ResponseData);

impl SerializableOutput for crate::service_apis::network::DeleteSubnetResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_get_floating_ip.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetFloatingIp;

#[derive(serde::Serialize)]
pub(crate) struct GetFloatingIpResponse(
    crate::service_apis::network::get_floating_ip::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::GetFloatingIpResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_allocate_floating_ip.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct AllocateFloatingIp;

#[derive(serde::Serialize)]
pub(crate) struct AllocateFloatingIpResponse(
    crate::service_apis::network::allocate_floating_ip::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::AllocateFloatingIpResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_associate_floating_ip.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct AssociateFloatingIp;

#[derive(serde::Serialize)]
pub(crate) struct AssociateFloatingIpResponse(
    crate::service_apis::network::associate_floating_ip::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::AssociateFloatingIpResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_disassociate_floating_ip.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DisassociateFloatingIp;

#[derive(serde::Serialize)]
pub(crate) struct DisassociateFloatingIpResponse(
    crate::service_apis::network::disassociate_floating_ip::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::DisassociateFloatingIpResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_release_floating_ip.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ReleaseFloatingIp;

#[derive(serde::Serialize)]
pub(crate) struct ReleaseFloatingIpResponse(
    crate::service_apis::network::release_floating_ip::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::ReleaseFloatingIpResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_get_vpc.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetVpc;

#[derive(serde::Serialize)]
pub(crate) struct GetVpcResponse(crate::service_apis::network::get_vpc::ResponseData);

impl SerializableOutput for crate::service_apis::network::GetVpcResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_get_subnet.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetSubnet;

#[derive(serde::Serialize)]
pub(crate) struct GetSubnetResponse(crate::service_apis::network::get_subnet::ResponseData);

impl SerializableOutput for crate::service_apis::network::GetSubnetResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_get_security_group.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetSecurityGroup;

#[derive(serde::Serialize)]
pub(crate) struct GetSecurityGroupResponse(
    crate::service_apis::network::get_security_group::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::GetSecurityGroupResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_list_security_groups.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListSecurityGroups;

#[derive(serde::Serialize)]
pub(crate) struct ListSecurityGroupsResponse(
    crate::service_apis::network::list_security_groups::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::ListSecurityGroupsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_create_security_group.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CreateSecurityGroup;

#[derive(serde::Serialize)]
pub(crate) struct CreateSecurityGroupResponse(
    crate::service_apis::network::create_security_group::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::CreateSecurityGroupResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_delete_security_group.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DeleteSecurityGroup;

#[derive(serde::Serialize)]
pub(crate) struct DeleteSecurityGroupResponse(
    crate::service_apis::network::delete_security_group::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::DeleteSecurityGroupResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_get_dns_policy.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetDnsPolicy;

#[derive(serde::Serialize)]
pub(crate) struct GetDnsPolicyResponse(crate::service_apis::network::get_dns_policy::ResponseData);

impl SerializableOutput for crate::service_apis::network::GetDnsPolicyResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_list_dns_policies.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListDnsPolicies;

#[derive(serde::Serialize)]
pub(crate) struct ListDnsPoliciesResponse(
    crate::service_apis::network::list_dns_policies::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::ListDnsPoliciesResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_create_dns_policy.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CreateDnsPolicy;

#[derive(serde::Serialize)]
pub(crate) struct CreateDnsPolicyResponse(
    crate::service_apis::network::create_dns_policy::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::CreateDnsPolicyResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_update_dns_policy.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct UpdateDnsPolicy;

#[derive(serde::Serialize)]
pub(crate) struct UpdateDnsPolicyResponse(
    crate::service_apis::network::update_dns_policy::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::UpdateDnsPolicyResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_set_default_dns_policy.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct SetDefaultDnsPolicy;

#[derive(serde::Serialize)]
pub(crate) struct SetDefaultDnsPolicyResponse(
    crate::service_apis::network::set_default_dns_policy::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::SetDefaultDnsPolicyResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_delete_dns_policy.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DeleteDnsPolicy;

#[derive(serde::Serialize)]
pub(crate) struct DeleteDnsPolicyResponse(
    crate::service_apis::network::delete_dns_policy::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::DeleteDnsPolicyResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_attach_dns_policy.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct AttachDnsPolicy;

#[derive(serde::Serialize)]
pub(crate) struct AttachDnsPolicyResponse(
    crate::service_apis::network::attach_dns_policy::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::AttachDnsPolicyResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_get_acl.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetAcl;

#[derive(serde::Serialize)]
pub(crate) struct GetAclResponse(crate::service_apis::network::get_acl::ResponseData);

impl SerializableOutput for crate::service_apis::network::GetAclResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/query_list_acls.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListAcls;

#[derive(serde::Serialize)]
pub(crate) struct ListAclsResponse(crate::service_apis::network::list_acls::ResponseData);

impl SerializableOutput for crate::service_apis::network::ListAclsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

macro_rules! route_table_query {
    ($ty:ident, $resp:ident, $module:ident, $file:literal) => {
        #[derive(GraphQLQuery)]
        #[graphql(
                                                            schema_path = "graphql/schema.graphql",
                                                            query_path = $file,
                                                            response_derives = "Serialize"
                                                        )]
        pub(crate) struct $ty;

        #[derive(serde::Serialize)]
        pub(crate) struct $resp(crate::service_apis::network::$module::ResponseData);

        impl SerializableOutput for $resp {
            fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
                serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
            }
        }
    };
}

route_table_query!(
    ListNatGateways,
    ListNatGatewaysResponse,
    list_nat_gateways,
    "graphql/network/query_list_nat_gateways.graphql"
);
route_table_query!(
    CreateNatGateway,
    CreateNatGatewayResponse,
    create_nat_gateway,
    "graphql/network/mutation_create_nat_gateway.graphql"
);
route_table_query!(
    DeleteNatGateway,
    DeleteNatGatewayResponse,
    delete_nat_gateway,
    "graphql/network/mutation_delete_nat_gateway.graphql"
);

route_table_query!(
    ListRouteTables,
    ListRouteTablesResponse,
    list_route_tables,
    "graphql/network/query_list_route_tables.graphql"
);
route_table_query!(
    GetRouteTable,
    GetRouteTableResponse,
    get_route_table,
    "graphql/network/query_get_route_table.graphql"
);
route_table_query!(
    CreateRouteTable,
    CreateRouteTableResponse,
    create_route_table,
    "graphql/network/mutation_create_route_table.graphql"
);
route_table_query!(
    UpdateRouteTableRoutes,
    UpdateRouteTableRoutesResponse,
    update_route_table_routes,
    "graphql/network/mutation_update_route_table_routes.graphql"
);
route_table_query!(
    DeleteRouteTable,
    DeleteRouteTableResponse,
    delete_route_table,
    "graphql/network/mutation_delete_route_table.graphql"
);
route_table_query!(
    AssociateRouteTable,
    AssociateRouteTableResponse,
    associate_route_table,
    "graphql/network/mutation_associate_route_table.graphql"
);

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_create_acl.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CreateAcl;

#[derive(serde::Serialize)]
pub(crate) struct CreateAclResponse(crate::service_apis::network::create_acl::ResponseData);

impl SerializableOutput for crate::service_apis::network::CreateAclResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_delete_acl.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DeleteAcl;

#[derive(serde::Serialize)]
pub(crate) struct DeleteAclResponse(crate::service_apis::network::delete_acl::ResponseData);

impl SerializableOutput for crate::service_apis::network::DeleteAclResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_update_subnet_acl.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct UpdateSubnetAcl;

#[derive(serde::Serialize)]
pub(crate) struct UpdateSubnetAclResponse(
    crate::service_apis::network::update_subnet_acl::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::UpdateSubnetAclResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_update_subnet_auto_assign_public_ip.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct UpdateSubnetAutoAssignPublicIp;

#[derive(serde::Serialize)]
pub(crate) struct UpdateSubnetAutoAssignPublicIpResponse(
    crate::service_apis::network::update_subnet_auto_assign_public_ip::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::UpdateSubnetAutoAssignPublicIpResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_update_security_group_rules.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct UpdateSecurityGroupRules;

#[derive(serde::Serialize)]
pub(crate) struct UpdateSecurityGroupRulesResponse(
    crate::service_apis::network::update_security_group_rules::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::UpdateSecurityGroupRulesResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_update_acl_rules.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct UpdateAclRules;

#[derive(serde::Serialize)]
pub(crate) struct UpdateAclRulesResponse(
    crate::service_apis::network::update_acl_rules::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::UpdateAclRulesResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_create_network_interface.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CreateNetworkInterface;

#[derive(serde::Serialize)]
pub(crate) struct CreateNetworkInterfaceResponse(
    crate::service_apis::network::create_network_interface::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::CreateNetworkInterfaceResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_set_network_interface_source_address_check.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct SetNetworkInterfaceSourceAddressCheck;

#[derive(serde::Serialize)]
pub(crate) struct SetNetworkInterfaceSourceAddressCheckResponse(
    crate::service_apis::network::set_network_interface_source_address_check::ResponseData,
);

impl SerializableOutput
    for crate::service_apis::network::SetNetworkInterfaceSourceAddressCheckResponse
{
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_set_network_interface_security_groups.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct SetNetworkInterfaceSecurityGroups;

#[derive(serde::Serialize)]
pub(crate) struct SetNetworkInterfaceSecurityGroupsResponse(
    crate::service_apis::network::set_network_interface_security_groups::ResponseData,
);

impl SerializableOutput
    for crate::service_apis::network::SetNetworkInterfaceSecurityGroupsResponse
{
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_set_resource_tags.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct SetResourceTags;

#[derive(serde::Serialize)]
pub(crate) struct SetResourceTagsResponse(
    crate::service_apis::network::set_resource_tags::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::SetResourceTagsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/network/mutation_remove_resource_tags.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct RemoveResourceTags;

#[derive(serde::Serialize)]
pub(crate) struct RemoveResourceTagsResponse(
    crate::service_apis::network::remove_resource_tags::ResponseData,
);

impl SerializableOutput for crate::service_apis::network::RemoveResourceTagsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

pub(crate) struct NetworkServiceClient {
    client: GenericGraphQLClient<ConfigAuthenticator>,
}

impl NetworkServiceClient {
    pub(crate) fn new(api_url: Url) -> Self {
        Self {
            client: GenericGraphQLClient::new(api_url, ConfigAuthenticator::default())
                .retrying(Retries::default()),
        }
    }

    pub(crate) async fn create_vpc(
        &self,
        project_urn: Urn,
        ipv4_cidr: Option<String>,
        tags: Vec<TagArg>,
    ) -> Result<CreateVpcResponse> {
        let variables = create_vpc::Variables {
            input: create_vpc::NetworkCreateVpcInput {
                project_urn,
                ipv4_cidr,
                tags: tag_inputs!(create_vpc, tags),
            },
        };
        let response_data = self.client.call::<CreateVpc>(variables).await?;
        Ok(CreateVpcResponse(response_data))
    }

    pub(crate) async fn set_resource_tags(
        &self,
        resource_urn: Urn,
        tags: Vec<TagArg>,
    ) -> Result<SetResourceTagsResponse> {
        let variables = set_resource_tags::Variables {
            input: set_resource_tags::NetworkSetResourceTagsInput {
                resource_urn,
                tags: tags
                    .into_iter()
                    .map(|tag| set_resource_tags::ResourceTagInput {
                        key: tag.key,
                        value: tag.value,
                    })
                    .collect(),
            },
        };
        let response_data = self.client.call::<SetResourceTags>(variables).await?;
        Ok(SetResourceTagsResponse(response_data))
    }

    pub(crate) async fn remove_resource_tags(
        &self,
        resource_urn: Urn,
        keys: Vec<String>,
    ) -> Result<RemoveResourceTagsResponse> {
        let variables = remove_resource_tags::Variables {
            input: remove_resource_tags::NetworkRemoveResourceTagsInput { resource_urn, keys },
        };
        let response_data = self.client.call::<RemoveResourceTags>(variables).await?;
        Ok(RemoveResourceTagsResponse(response_data))
    }

    pub(crate) async fn list_vpcs(&self, project_urn: Urn) -> Result<ListVpcsResponse> {
        let variables = list_vpcs::Variables {
            input: list_vpcs::NetworkListVpcsInput { project_urn },
        };

        let response_data = self.client.call::<ListVpcs>(variables).await?;
        Ok(ListVpcsResponse(response_data))
    }

    pub(crate) async fn create_subnet(
        &self,
        vpc_urn: Urn,
        auto_assign_public_ip: bool,
        tags: Vec<TagArg>,
    ) -> Result<CreateSubnetResponse> {
        let variables = create_subnet::Variables {
            input: create_subnet::NetworkCreateSubnetInput {
                vpc_urn,
                auto_assign_public_ip,
                tags: tag_inputs!(create_subnet, tags),
            },
        };
        let response_data = self.client.call::<CreateSubnet>(variables).await?;
        Ok(CreateSubnetResponse(response_data))
    }

    pub(crate) async fn list_subnets(&self, project_urn: Urn) -> Result<ListSubnetsResponse> {
        let variables = list_subnets::Variables {
            input: list_subnets::NetworkListSubnetsInput { project_urn },
        };

        let response_data = self.client.call::<ListSubnets>(variables).await?;
        Ok(ListSubnetsResponse(response_data))
    }

    pub(crate) async fn get_network_interface(
        &self,
        network_interface_urn: Urn,
    ) -> Result<GetNetworkInterfaceResponse> {
        let variables = get_network_interface::Variables {
            network_interface_urn,
        };
        let response_data = self.client.call::<GetNetworkInterface>(variables).await?;
        Ok(GetNetworkInterfaceResponse(response_data))
    }

    pub(crate) async fn delete_network_interface(
        &self,
        _confirmed: crate::confirm::Confirmed,
        network_interface_urn: Urn,
    ) -> Result<DeleteNetworkInterfaceResponse> {
        let variables = delete_network_interface::Variables {
            input: delete_network_interface::NetworkDeleteNetworkInterfaceInput {
                network_interface_urn,
            },
        };
        let response_data = self
            .client
            .call::<DeleteNetworkInterface>(variables)
            .await?;
        Ok(DeleteNetworkInterfaceResponse(response_data))
    }

    pub(crate) async fn list_network_interfaces(
        &self,
        project_urn: Urn,
    ) -> Result<ListNetworkInterfacesResponse> {
        let variables = list_network_interfaces::Variables {
            input: list_network_interfaces::NetworkListNetworkInterfacesInput { project_urn },
        };
        let response_data = self.client.call::<ListNetworkInterfaces>(variables).await?;
        Ok(ListNetworkInterfacesResponse(response_data))
    }

    pub(crate) async fn list_floating_ips(
        &self,
        project_urn: Urn,
    ) -> Result<ListFloatingIpsResponse> {
        let variables = list_floating_ips::Variables {
            input: list_floating_ips::NetworkListFloatingIpsInput { project_urn },
        };
        let response_data = self.client.call::<ListFloatingIps>(variables).await?;
        Ok(ListFloatingIpsResponse(response_data))
    }

    pub(crate) async fn get_internet_gateway(
        &self,
        vpc_urn: Urn,
    ) -> Result<GetInternetGatewayResponse> {
        let variables = get_internet_gateway::Variables { vpc_urn };
        let response_data = self.client.call::<GetInternetGateway>(variables).await?;
        Ok(GetInternetGatewayResponse(response_data))
    }

    pub(crate) async fn attach_internet_gateway(
        &self,
        vpc_urn: Urn,
        tags: Vec<TagArg>,
    ) -> Result<AttachInternetGatewayResponse> {
        let variables = attach_internet_gateway::Variables {
            input: attach_internet_gateway::NetworkAttachInternetGatewayInput {
                vpc_urn,
                tags: tag_inputs!(attach_internet_gateway, tags),
            },
        };
        let response_data = self.client.call::<AttachInternetGateway>(variables).await?;
        Ok(AttachInternetGatewayResponse(response_data))
    }

    pub(crate) async fn detach_internet_gateway(
        &self,
        vpc_urn: Urn,
    ) -> Result<DetachInternetGatewayResponse> {
        let variables = detach_internet_gateway::Variables {
            input: detach_internet_gateway::NetworkDetachInternetGatewayInput { vpc_urn },
        };
        let response_data = self.client.call::<DetachInternetGateway>(variables).await?;
        Ok(DetachInternetGatewayResponse(response_data))
    }

    pub(crate) async fn delete_vpc(
        &self,
        _confirmed: crate::confirm::Confirmed,
        vpc_urn: Urn,
    ) -> Result<DeleteVpcResponse> {
        let variables = delete_vpc::Variables {
            input: delete_vpc::NetworkDeleteVpcInput { vpc_urn },
        };
        let response_data = self.client.call::<DeleteVpc>(variables).await?;
        Ok(DeleteVpcResponse(response_data))
    }

    pub(crate) async fn delete_subnet(
        &self,
        _confirmed: crate::confirm::Confirmed,
        subnet_urn: Urn,
    ) -> Result<DeleteSubnetResponse> {
        let variables = delete_subnet::Variables {
            input: delete_subnet::NetworkDeleteSubnetInput { subnet_urn },
        };
        let response_data = self.client.call::<DeleteSubnet>(variables).await?;
        Ok(DeleteSubnetResponse(response_data))
    }

    pub(crate) async fn get_floating_ip(
        &self,
        floating_ip_urn: Urn,
    ) -> Result<GetFloatingIpResponse> {
        let variables = get_floating_ip::Variables { floating_ip_urn };
        let response_data = self.client.call::<GetFloatingIp>(variables).await?;
        Ok(GetFloatingIpResponse(response_data))
    }

    pub(crate) async fn allocate_floating_ip(
        &self,
        project_urn: Urn,
        tags: Vec<TagArg>,
    ) -> Result<AllocateFloatingIpResponse> {
        let variables = allocate_floating_ip::Variables {
            input: allocate_floating_ip::NetworkAllocateFloatingIpInput {
                project_urn,
                tags: tag_inputs!(allocate_floating_ip, tags),
            },
        };
        let response_data = self.client.call::<AllocateFloatingIp>(variables).await?;
        Ok(AllocateFloatingIpResponse(response_data))
    }

    pub(crate) async fn associate_floating_ip(
        &self,
        floating_ip_urn: Urn,
        network_interface_urn: Urn,
    ) -> Result<AssociateFloatingIpResponse> {
        let variables = associate_floating_ip::Variables {
            input: associate_floating_ip::NetworkAssociateFloatingIpInput {
                floating_ip_urn,
                network_interface_urn,
            },
        };
        let response_data = self.client.call::<AssociateFloatingIp>(variables).await?;
        Ok(AssociateFloatingIpResponse(response_data))
    }

    pub(crate) async fn disassociate_floating_ip(
        &self,
        floating_ip_urn: Urn,
    ) -> Result<DisassociateFloatingIpResponse> {
        let variables = disassociate_floating_ip::Variables {
            input: disassociate_floating_ip::NetworkDisassociateFloatingIpInput { floating_ip_urn },
        };
        let response_data = self
            .client
            .call::<DisassociateFloatingIp>(variables)
            .await?;
        Ok(DisassociateFloatingIpResponse(response_data))
    }

    pub(crate) async fn release_floating_ip(
        &self,
        _confirmed: crate::confirm::Confirmed,
        floating_ip_urn: Urn,
    ) -> Result<ReleaseFloatingIpResponse> {
        let variables = release_floating_ip::Variables {
            input: release_floating_ip::NetworkReleaseFloatingIpInput { floating_ip_urn },
        };
        let response_data = self.client.call::<ReleaseFloatingIp>(variables).await?;
        Ok(ReleaseFloatingIpResponse(response_data))
    }

    pub(crate) async fn get_vpc(&self, vpc_urn: Urn) -> Result<GetVpcResponse> {
        let variables = get_vpc::Variables { vpc_urn };
        let response_data = self.client.call::<GetVpc>(variables).await?;
        Ok(GetVpcResponse(response_data))
    }

    pub(crate) async fn get_subnet(&self, subnet_urn: Urn) -> Result<GetSubnetResponse> {
        let variables = get_subnet::Variables { subnet_urn };
        let response_data = self.client.call::<GetSubnet>(variables).await?;
        Ok(GetSubnetResponse(response_data))
    }

    pub(crate) async fn get_security_group(
        &self,
        security_group_urn: Urn,
    ) -> Result<GetSecurityGroupResponse> {
        let variables = get_security_group::Variables { security_group_urn };
        let response_data = self.client.call::<GetSecurityGroup>(variables).await?;
        Ok(GetSecurityGroupResponse(response_data))
    }

    pub(crate) async fn list_security_groups(
        &self,
        project_urn: Urn,
    ) -> Result<ListSecurityGroupsResponse> {
        let variables = list_security_groups::Variables {
            input: list_security_groups::NetworkListSecurityGroupsInput { project_urn },
        };
        let response_data = self.client.call::<ListSecurityGroups>(variables).await?;
        Ok(ListSecurityGroupsResponse(response_data))
    }

    pub(crate) async fn create_security_group(
        &self,
        vpc_urn: Urn,
        tags: Vec<TagArg>,
    ) -> Result<CreateSecurityGroupResponse> {
        let variables = create_security_group::Variables {
            input: create_security_group::NetworkCreateSecurityGroupInput {
                vpc_urn,
                ingress_rules: vec![],
                egress_rules: vec![],
                tags: tag_inputs!(create_security_group, tags),
            },
        };
        let response_data = self.client.call::<CreateSecurityGroup>(variables).await?;
        Ok(CreateSecurityGroupResponse(response_data))
    }

    pub(crate) async fn delete_security_group(
        &self,
        _confirmed: crate::confirm::Confirmed,
        security_group_urn: Urn,
    ) -> Result<DeleteSecurityGroupResponse> {
        let variables = delete_security_group::Variables {
            input: delete_security_group::NetworkDeleteSecurityGroupInput { security_group_urn },
        };
        let response_data = self.client.call::<DeleteSecurityGroup>(variables).await?;
        Ok(DeleteSecurityGroupResponse(response_data))
    }

    pub(crate) async fn get_dns_policy(&self, dns_policy_urn: Urn) -> Result<GetDnsPolicyResponse> {
        let variables = get_dns_policy::Variables { dns_policy_urn };
        let response_data = self.client.call::<GetDnsPolicy>(variables).await?;
        Ok(GetDnsPolicyResponse(response_data))
    }

    pub(crate) async fn list_dns_policies(
        &self,
        project_urn: Urn,
    ) -> Result<ListDnsPoliciesResponse> {
        let variables = list_dns_policies::Variables {
            input: list_dns_policies::NetworkListDnsPoliciesInput { project_urn },
        };
        let response_data = self.client.call::<ListDnsPolicies>(variables).await?;
        Ok(ListDnsPoliciesResponse(response_data))
    }

    pub(crate) async fn create_dns_policy(
        &self,
        project_urn: Urn,
        relay_enabled: bool,
        upstream_servers: Vec<String>,
    ) -> Result<CreateDnsPolicyResponse> {
        let variables = create_dns_policy::Variables {
            input: create_dns_policy::NetworkCreateDnsPolicyInput {
                project_urn,
                relay_enabled,
                upstream_servers,
            },
        };
        let response_data = self.client.call::<CreateDnsPolicy>(variables).await?;
        Ok(CreateDnsPolicyResponse(response_data))
    }

    pub(crate) async fn update_dns_policy(
        &self,
        dns_policy_urn: Urn,
        relay_enabled: bool,
        upstream_servers: Vec<String>,
    ) -> Result<UpdateDnsPolicyResponse> {
        let variables = update_dns_policy::Variables {
            input: update_dns_policy::NetworkUpdateDnsPolicyInput {
                dns_policy_urn,
                relay_enabled,
                upstream_servers,
            },
        };
        let response_data = self.client.call::<UpdateDnsPolicy>(variables).await?;
        Ok(UpdateDnsPolicyResponse(response_data))
    }

    pub(crate) async fn set_default_dns_policy(
        &self,
        dns_policy_urn: Urn,
    ) -> Result<SetDefaultDnsPolicyResponse> {
        let variables = set_default_dns_policy::Variables {
            input: set_default_dns_policy::NetworkSetDefaultDnsPolicyInput { dns_policy_urn },
        };
        let response_data = self.client.call::<SetDefaultDnsPolicy>(variables).await?;
        Ok(SetDefaultDnsPolicyResponse(response_data))
    }

    pub(crate) async fn delete_dns_policy(
        &self,
        _confirmed: crate::confirm::Confirmed,
        dns_policy_urn: Urn,
    ) -> Result<DeleteDnsPolicyResponse> {
        let variables = delete_dns_policy::Variables {
            input: delete_dns_policy::NetworkDeleteDnsPolicyInput { dns_policy_urn },
        };
        let response_data = self.client.call::<DeleteDnsPolicy>(variables).await?;
        Ok(DeleteDnsPolicyResponse(response_data))
    }

    pub(crate) async fn attach_dns_policy(
        &self,
        vpc_urn: Urn,
        dns_policy_urn: Urn,
    ) -> Result<AttachDnsPolicyResponse> {
        let variables = attach_dns_policy::Variables {
            input: attach_dns_policy::NetworkAttachDnsPolicyInput {
                vpc_urn,
                dns_policy_urn,
            },
        };
        let response_data = self.client.call::<AttachDnsPolicy>(variables).await?;
        Ok(AttachDnsPolicyResponse(response_data))
    }

    pub(crate) async fn get_acl(&self, acl_urn: Urn) -> Result<GetAclResponse> {
        let variables = get_acl::Variables { acl_urn };
        let response_data = self.client.call::<GetAcl>(variables).await?;
        Ok(GetAclResponse(response_data))
    }

    pub(crate) async fn list_acls(&self, project_urn: Urn) -> Result<ListAclsResponse> {
        let variables = list_acls::Variables {
            input: list_acls::NetworkListAclsInput { project_urn },
        };
        let response_data = self.client.call::<ListAcls>(variables).await?;
        Ok(ListAclsResponse(response_data))
    }

    pub(crate) async fn list_nat_gateways(
        &self,
        project_urn: Urn,
    ) -> Result<ListNatGatewaysResponse> {
        let variables = list_nat_gateways::Variables {
            input: list_nat_gateways::NetworkListNatGatewaysInput { project_urn },
        };
        let response_data = self.client.call::<ListNatGateways>(variables).await?;
        Ok(ListNatGatewaysResponse(response_data))
    }

    pub(crate) async fn create_nat_gateway(
        &self,
        subnet_urn: Urn,
        floating_ip_urn: Urn,
        tags: Vec<TagArg>,
    ) -> Result<CreateNatGatewayResponse> {
        let variables = create_nat_gateway::Variables {
            input: create_nat_gateway::NetworkCreateNatGatewayInput {
                subnet_urn,
                floating_ip_urn,
                // Omitted rather than chosen: the API defaults it, and a
                // flag whose only legal value is the default would be one
                // more thing to type and ignore.
                address_family: None,
                tags: tag_inputs!(create_nat_gateway, tags),
            },
        };
        let response_data = self.client.call::<CreateNatGateway>(variables).await?;
        Ok(CreateNatGatewayResponse(response_data))
    }

    pub(crate) async fn delete_nat_gateway(
        &self,
        _confirmed: crate::confirm::Confirmed,
        nat_gateway_urn: Urn,
    ) -> Result<DeleteNatGatewayResponse> {
        let variables = delete_nat_gateway::Variables {
            input: delete_nat_gateway::NetworkDeleteNatGatewayInput { nat_gateway_urn },
        };
        let response_data = self.client.call::<DeleteNatGateway>(variables).await?;
        Ok(DeleteNatGatewayResponse(response_data))
    }

    pub(crate) async fn list_route_tables(
        &self,
        project_urn: Urn,
    ) -> Result<ListRouteTablesResponse> {
        let variables = list_route_tables::Variables {
            input: list_route_tables::NetworkListRouteTablesInput { project_urn },
        };
        let response_data = self.client.call::<ListRouteTables>(variables).await?;
        Ok(ListRouteTablesResponse(response_data))
    }

    pub(crate) async fn get_route_table(
        &self,
        route_table_urn: Urn,
    ) -> Result<GetRouteTableResponse> {
        let variables = get_route_table::Variables {
            input: get_route_table::NetworkGetRouteTableInput { route_table_urn },
        };
        let response_data = self.client.call::<GetRouteTable>(variables).await?;
        Ok(GetRouteTableResponse(response_data))
    }

    pub(crate) async fn create_route_table(
        &self,
        vpc_urn: Urn,
        tags: Vec<TagArg>,
    ) -> Result<CreateRouteTableResponse> {
        let variables = create_route_table::Variables {
            input: create_route_table::NetworkCreateRouteTableInput {
                vpc_urn,
                tags: tag_inputs!(create_route_table, tags),
            },
        };
        let response_data = self.client.call::<CreateRouteTable>(variables).await?;
        Ok(CreateRouteTableResponse(response_data))
    }

    pub(crate) async fn update_route_table_routes(
        &self,
        route_table_urn: Urn,
        routes: Vec<update_route_table_routes::NetworkRouteTableRouteInput>,
    ) -> Result<UpdateRouteTableRoutesResponse> {
        let variables = update_route_table_routes::Variables {
            input: update_route_table_routes::NetworkUpdateRouteTableRoutesInput {
                route_table_urn,
                routes,
            },
        };
        let response_data = self
            .client
            .call::<UpdateRouteTableRoutes>(variables)
            .await?;
        Ok(UpdateRouteTableRoutesResponse(response_data))
    }

    pub(crate) async fn delete_route_table(
        &self,
        _confirmed: crate::confirm::Confirmed,
        route_table_urn: Urn,
    ) -> Result<DeleteRouteTableResponse> {
        let variables = delete_route_table::Variables {
            input: delete_route_table::NetworkDeleteRouteTableInput { route_table_urn },
        };
        let response_data = self.client.call::<DeleteRouteTable>(variables).await?;
        Ok(DeleteRouteTableResponse(response_data))
    }

    pub(crate) async fn associate_route_table(
        &self,
        subnet_urn: Urn,
        route_table_urn: Urn,
    ) -> Result<AssociateRouteTableResponse> {
        let variables = associate_route_table::Variables {
            input: associate_route_table::NetworkAssociateRouteTableInput {
                subnet_urn,
                route_table_urn,
            },
        };
        let response_data = self.client.call::<AssociateRouteTable>(variables).await?;
        Ok(AssociateRouteTableResponse(response_data))
    }

    pub(crate) async fn create_acl(
        &self,
        vpc_urn: Urn,
        tags: Vec<TagArg>,
    ) -> Result<CreateAclResponse> {
        let variables = create_acl::Variables {
            input: create_acl::NetworkCreateAclInput {
                vpc_urn,
                tags: tag_inputs!(create_acl, tags),
            },
        };
        let response_data = self.client.call::<CreateAcl>(variables).await?;
        Ok(CreateAclResponse(response_data))
    }

    pub(crate) async fn delete_acl(
        &self,
        _confirmed: crate::confirm::Confirmed,
        acl_urn: Urn,
    ) -> Result<DeleteAclResponse> {
        let variables = delete_acl::Variables {
            input: delete_acl::NetworkDeleteAclInput { acl_urn },
        };
        let response_data = self.client.call::<DeleteAcl>(variables).await?;
        Ok(DeleteAclResponse(response_data))
    }

    pub(crate) async fn update_subnet_acl(
        &self,
        subnet_urn: Urn,
        acl_urn: Urn,
    ) -> Result<UpdateSubnetAclResponse> {
        let variables = update_subnet_acl::Variables {
            input: update_subnet_acl::NetworkUpdateSubnetAclInput {
                subnet_urn,
                acl_urn,
            },
        };
        let response_data = self.client.call::<UpdateSubnetAcl>(variables).await?;
        Ok(UpdateSubnetAclResponse(response_data))
    }

    pub(crate) async fn update_subnet_auto_assign_public_ip(
        &self,
        subnet_urn: Urn,
        auto_assign_public_ip: bool,
    ) -> Result<UpdateSubnetAutoAssignPublicIpResponse> {
        let variables = update_subnet_auto_assign_public_ip::Variables {
            input:
                update_subnet_auto_assign_public_ip::NetworkUpdateSubnetAutoAssignPublicIpInput {
                    subnet_urn,
                    auto_assign_public_ip,
                },
        };
        let response_data = self
            .client
            .call::<UpdateSubnetAutoAssignPublicIp>(variables)
            .await?;
        Ok(UpdateSubnetAutoAssignPublicIpResponse(response_data))
    }

    pub(crate) async fn create_network_interface(
        &self,
        project_urn: Urn,
        subnet_urn: Urn,
        security_group_urns: Vec<Urn>,
        source_address_check: bool,
        tags: Vec<TagArg>,
    ) -> Result<CreateNetworkInterfaceResponse> {
        let security_group_urns = if security_group_urns.is_empty() {
            None
        } else {
            Some(security_group_urns)
        };
        let variables = create_network_interface::Variables {
            input: create_network_interface::NetworkCreateNetworkInterfaceInput {
                project_urn,
                subnet_urn,
                security_group_urns,
                source_address_check,
                tags: tag_inputs!(create_network_interface, tags),
            },
        };
        let response_data = self
            .client
            .call::<CreateNetworkInterface>(variables)
            .await?;
        Ok(CreateNetworkInterfaceResponse(response_data))
    }

    pub(crate) async fn set_network_interface_source_address_check(
        &self,
        network_interface_urn: Urn,
        source_address_check: bool,
    ) -> Result<SetNetworkInterfaceSourceAddressCheckResponse> {
        let variables = set_network_interface_source_address_check::Variables {
            input: set_network_interface_source_address_check::NetworkSetNetworkInterfaceSourceAddressCheckInput {
                network_interface_urn,
                source_address_check,
            },
        };
        let response_data = self
            .client
            .call::<SetNetworkInterfaceSourceAddressCheck>(variables)
            .await?;
        Ok(SetNetworkInterfaceSourceAddressCheckResponse(response_data))
    }

    pub(crate) async fn set_network_interface_security_groups(
        &self,
        network_interface_urn: Urn,
        security_group_urns: Vec<Urn>,
    ) -> Result<SetNetworkInterfaceSecurityGroupsResponse> {
        let variables = set_network_interface_security_groups::Variables {
            input: set_network_interface_security_groups::NetworkSetNetworkInterfaceSecurityGroupsInput {
                network_interface_urn,
                security_group_urns,
            },
        };
        let response_data = self
            .client
            .call::<SetNetworkInterfaceSecurityGroups>(variables)
            .await?;
        Ok(SetNetworkInterfaceSecurityGroupsResponse(response_data))
    }

    pub(crate) async fn update_acl_rules(
        &self,
        acl_urn: Urn,
        rules: Vec<AclRuleArg>,
    ) -> Result<UpdateAclRulesResponse> {
        let rules = rules
            .into_iter()
            .map(|rule| update_acl_rules::NetworkAclRuleInput {
                priority: rule.priority as i64,
                direction: match rule.direction {
                    AclDirectionArg::Ingress => {
                        update_acl_rules::NetworkAclRuleDirectionInput::INGRESS
                    }
                    AclDirectionArg::Egress => {
                        update_acl_rules::NetworkAclRuleDirectionInput::EGRESS
                    }
                },
                protocol: match rule.protocol {
                    ProtocolArg::Any => update_acl_rules::NetworkProtocolInput::ANY,
                    ProtocolArg::Tcp => update_acl_rules::NetworkProtocolInput::TCP,
                    ProtocolArg::Udp => update_acl_rules::NetworkProtocolInput::UDP,
                    ProtocolArg::Icmp => update_acl_rules::NetworkProtocolInput::ICMP,
                },
                port: update_acl_rules::NetworkAclPortInput {
                    from: rule.port_from.map(i64::from),
                    to: rule.port_to.map(i64::from),
                },
                source: rule.source,
                policy: match rule.policy {
                    AclPolicyArg::Accept => update_acl_rules::NetworkAclRulePolicyInput::ACCEPT,
                    AclPolicyArg::Drop => update_acl_rules::NetworkAclRulePolicyInput::DROP,
                },
            })
            .collect();
        let variables = update_acl_rules::Variables {
            input: update_acl_rules::NetworkUpdateAclRulesInput { acl_urn, rules },
        };
        let response_data = self.client.call::<UpdateAclRules>(variables).await?;
        Ok(UpdateAclRulesResponse(response_data))
    }

    pub(crate) async fn update_security_group_rules(
        &self,
        security_group_urn: Urn,
        ingress_rules: Vec<SecurityGroupRuleArg>,
        egress_rules: Vec<SecurityGroupRuleArg>,
    ) -> Result<UpdateSecurityGroupRulesResponse> {
        let map_rule = |rule: SecurityGroupRuleArg| {
            update_security_group_rules::NetworkSecurityGroupRuleInput {
                protocol: match rule.protocol {
                    ProtocolArg::Any => update_security_group_rules::NetworkProtocolInput::ANY,
                    ProtocolArg::Tcp => update_security_group_rules::NetworkProtocolInput::TCP,
                    ProtocolArg::Udp => update_security_group_rules::NetworkProtocolInput::UDP,
                    ProtocolArg::Icmp => update_security_group_rules::NetworkProtocolInput::ICMP,
                },
                port: update_security_group_rules::NetworkAclPortInput {
                    from: rule.port_from.map(i64::from),
                    to: rule.port_to.map(i64::from),
                },
                peer: update_security_group_rules::NetworkSecurityGroupRulePeerInput {
                    ip_range: rule.ip_range,
                    security_group_urn: rule.security_group_urn,
                },
                description: rule.description,
            }
        };
        let variables = update_security_group_rules::Variables {
            input: update_security_group_rules::NetworkUpdateSecurityGroupRulesInput {
                security_group_urn,
                ingress_rules: ingress_rules.into_iter().map(map_rule).collect(),
                egress_rules: egress_rules.into_iter().map(map_rule).collect(),
            },
        };
        let response_data = self
            .client
            .call::<UpdateSecurityGroupRules>(variables)
            .await?;
        Ok(UpdateSecurityGroupRulesResponse(response_data))
    }
}
