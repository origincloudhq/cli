use crate::client_auth::ConfigAuthenticator;
use crate::graphql::Error as ClientError;
use crate::graphql::GenericGraphQLClient;
#[allow(unused_imports)]
use crate::identity::*;
use crate::output::{Listing, SerializableOutput, SerializationError, Shape};
use graphql_client::*;
use std::str::FromStr;
use url::Url;

pub type DateTime = chrono::DateTime<chrono::Utc>;
pub type Result<T> = std::result::Result<T, ClientError>;

/// The list in the GraphQL response's `field` (`iam.orgMembers`), as the
/// human formats print it: `table` with `columns`, JSON paths from an item,
/// such as `user.email`; `wide` with every other field that fits in a cell
/// after them; an empty one suggesting `create`.
fn listing(
    field: &'static str,
    columns: &'static [&'static str],
    create: Option<&'static str>,
) -> Shape {
    Shape::List(Listing {
        field,
        columns,
        create,
        ..Listing::default()
    })
}

/// A permission's stable identifier, e.g. `network.vpc.list`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(crate) struct PermissionKey(String);

impl std::fmt::Display for PermissionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for PermissionKey {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/mutation_assign_role.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct AssignRole;

#[derive(serde::Serialize)]
pub(crate) struct AssignRoleResponse(assign_role::ResponseData);

impl SerializableOutput for AssignRoleResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/mutation_revoke_role.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct RevokeRole;

#[derive(serde::Serialize)]
pub(crate) struct RevokeRoleResponse(revoke_role::ResponseData);

impl SerializableOutput for RevokeRoleResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/mutation_invite_member.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct InviteMember;

#[derive(serde::Serialize)]
pub(crate) struct InviteMemberResponse(invite_member::ResponseData);

impl SerializableOutput for InviteMemberResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/mutation_remove_member.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct RemoveMember;

#[derive(serde::Serialize)]
pub(crate) struct RemoveMemberResponse(remove_member::ResponseData);

impl SerializableOutput for RemoveMemberResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/mutation_revoke_invite.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct RevokeInvite;

#[derive(serde::Serialize)]
pub(crate) struct RevokeInviteResponse(revoke_invite::ResponseData);

impl SerializableOutput for RevokeInviteResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/query_org_members.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct OrgMembers;

#[derive(serde::Serialize)]
pub(crate) struct OrgMembersResponse(org_members::ResponseData);

impl SerializableOutput for OrgMembersResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }

    fn shape(&self) -> Shape {
        listing(
            "iam.orgMembers",
            &[
                "user.email",
                "user.firstName",
                "user.lastName",
                "roleUrns",
                "user.enabled",
            ],
            None,
        )
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/query_org_invites.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct OrgInvites;

#[derive(serde::Serialize)]
pub(crate) struct OrgInvitesResponse(org_invites::ResponseData);

impl SerializableOutput for OrgInvitesResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }

    fn shape(&self) -> Shape {
        listing(
            "iam.orgInvites",
            &["email", "roleUrn", "invitedBy", "createdAt"],
            None,
        )
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/mutation_create_service_account.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CreateServiceAccount;

#[derive(serde::Serialize)]
pub(crate) struct CreateServiceAccountResponse(create_service_account::ResponseData);

impl SerializableOutput for CreateServiceAccountResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/mutation_revoke_service_account.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct RevokeServiceAccount;

#[derive(serde::Serialize)]
pub(crate) struct RevokeServiceAccountResponse(revoke_service_account::ResponseData);

impl SerializableOutput for RevokeServiceAccountResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/query_service_accounts.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ServiceAccounts;

#[derive(serde::Serialize)]
pub(crate) struct ServiceAccountsResponse(service_accounts::ResponseData);

impl SerializableOutput for ServiceAccountsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }

    fn shape(&self) -> Shape {
        listing(
            "iam.serviceAccounts",
            &["name", "principal", "createdAt", "revokedAt"],
            Some("origin iam create-service-account"),
        )
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/query_current_user.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CurrentUser;

#[derive(serde::Serialize)]
pub(crate) struct CurrentUserResponse(current_user::ResponseData);

impl SerializableOutput for CurrentUserResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/query_current_user_grants.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CurrentUserGrants;

#[derive(serde::Serialize)]
pub(crate) struct CurrentUserGrantsResponse(current_user_grants::ResponseData);

impl SerializableOutput for CurrentUserGrantsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }

    fn shape(&self) -> Shape {
        listing(
            "iam.currentUserGrants",
            &["orgId", "projectId", "roles.roleUrn", "createdAt"],
            None,
        )
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/query_principal_grants.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct PrincipalGrants;

#[derive(serde::Serialize)]
pub(crate) struct PrincipalGrantsResponse(principal_grants::ResponseData);

impl SerializableOutput for PrincipalGrantsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }

    fn shape(&self) -> Shape {
        listing(
            "iam.principalGrants",
            &["principal", "projectId", "roles.roleUrn", "createdAt"],
            None,
        )
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/query_list_roles.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListRoles;

#[derive(serde::Serialize)]
pub(crate) struct ListRolesResponse(list_roles::ResponseData);

impl SerializableOutput for ListRolesResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }

    fn shape(&self) -> Shape {
        listing("iam.roles", &["name", "description"], None)
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/query_get_role.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetRole;

#[derive(serde::Serialize)]
pub(crate) struct GetRoleResponse(get_role::ResponseData);

impl SerializableOutput for GetRoleResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/query_permissions.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct Permissions;

#[derive(serde::Serialize)]
pub(crate) struct PermissionsResponse(permissions::ResponseData);

impl SerializableOutput for PermissionsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }

    fn shape(&self) -> Shape {
        listing(
            "iam.permissions",
            &["key", "service", "resourceType", "action"],
            None,
        )
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/query_has_permission.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct HasPermission;

#[derive(serde::Serialize)]
pub(crate) struct HasPermissionResponse(has_permission::ResponseData);

impl SerializableOutput for HasPermissionResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

pub(crate) struct IamServiceClient {
    client: GenericGraphQLClient<ConfigAuthenticator>,
}

impl IamServiceClient {
    pub(crate) fn new(api_url: Url) -> Self {
        Self {
            client: GenericGraphQLClient::new(api_url, ConfigAuthenticator::default()),
        }
    }

    pub(crate) async fn assign_role(
        &self,
        org_id: OrgId,
        project_id: Option<ProjectId>,
        principal: Principal,
        role_urn: Urn,
    ) -> Result<AssignRoleResponse> {
        let variables = assign_role::Variables {
            input: assign_role::IamRoleAssignmentInput {
                org_id,
                project_id,
                principal,
                role_urn,
            },
        };
        let response_data = self.client.call::<AssignRole>(variables).await?;
        Ok(AssignRoleResponse(response_data))
    }

    pub(crate) async fn revoke_role(
        &self,
        org_id: OrgId,
        project_id: Option<ProjectId>,
        principal: Principal,
        role_urn: Urn,
    ) -> Result<RevokeRoleResponse> {
        let variables = revoke_role::Variables {
            input: revoke_role::IamRoleAssignmentInput {
                org_id,
                project_id,
                principal,
                role_urn,
            },
        };
        let response_data = self.client.call::<RevokeRole>(variables).await?;
        Ok(RevokeRoleResponse(response_data))
    }

    pub(crate) async fn invite_member(
        &self,
        org_id: OrgId,
        email: String,
        role_urn: Urn,
    ) -> Result<InviteMemberResponse> {
        let variables = invite_member::Variables {
            input: invite_member::IamInviteMemberInput {
                org_id,
                email,
                role_urn,
            },
        };
        let response_data = self.client.call::<InviteMember>(variables).await?;
        Ok(InviteMemberResponse(response_data))
    }

    pub(crate) async fn remove_member(
        &self,
        org_id: OrgId,
        principal: Principal,
    ) -> Result<RemoveMemberResponse> {
        let variables = remove_member::Variables {
            input: remove_member::IamRemoveMemberInput { org_id, principal },
        };
        let response_data = self.client.call::<RemoveMember>(variables).await?;
        Ok(RemoveMemberResponse(response_data))
    }

    pub(crate) async fn revoke_invite(
        &self,
        org_id: OrgId,
        invite_id: String,
    ) -> Result<RevokeInviteResponse> {
        let variables = revoke_invite::Variables {
            input: revoke_invite::IamRevokeInviteInput { org_id, invite_id },
        };
        let response_data = self.client.call::<RevokeInvite>(variables).await?;
        Ok(RevokeInviteResponse(response_data))
    }

    pub(crate) async fn org_members(&self, org_id: OrgId) -> Result<OrgMembersResponse> {
        let variables = org_members::Variables {
            input: org_members::IamOrgMembersInput { org_id },
        };
        let response_data = self.client.call::<OrgMembers>(variables).await?;
        Ok(OrgMembersResponse(response_data))
    }

    pub(crate) async fn org_invites(&self, org_id: OrgId) -> Result<OrgInvitesResponse> {
        let variables = org_invites::Variables {
            input: org_invites::IamOrgInvitesInput { org_id },
        };
        let response_data = self.client.call::<OrgInvites>(variables).await?;
        Ok(OrgInvitesResponse(response_data))
    }

    pub(crate) async fn create_service_account(
        &self,
        org_id: OrgId,
        name: String,
    ) -> Result<CreateServiceAccountResponse> {
        let variables = create_service_account::Variables {
            input: create_service_account::IamCreateServiceAccountInput { org_id, name },
        };
        let response_data = self.client.call::<CreateServiceAccount>(variables).await?;
        Ok(CreateServiceAccountResponse(response_data))
    }

    pub(crate) async fn revoke_service_account(
        &self,
        _confirmed: crate::confirm::Confirmed,
        org_id: OrgId,
        service_account_id: String,
    ) -> Result<RevokeServiceAccountResponse> {
        let variables = revoke_service_account::Variables {
            input: revoke_service_account::IamRevokeServiceAccountInput {
                org_id,
                service_account_id,
            },
        };
        let response_data = self.client.call::<RevokeServiceAccount>(variables).await?;
        Ok(RevokeServiceAccountResponse(response_data))
    }

    pub(crate) async fn service_accounts(&self, org_id: OrgId) -> Result<ServiceAccountsResponse> {
        let variables = service_accounts::Variables {
            input: service_accounts::IamServiceAccountsInput { org_id },
        };
        let response_data = self.client.call::<ServiceAccounts>(variables).await?;
        Ok(ServiceAccountsResponse(response_data))
    }

    pub(crate) async fn current_user(&self) -> Result<CurrentUserResponse> {
        let response_data = self
            .client
            .call::<CurrentUser>(current_user::Variables)
            .await?;
        Ok(CurrentUserResponse(response_data))
    }

    pub(crate) async fn current_user_grants(&self) -> Result<CurrentUserGrantsResponse> {
        let response_data = self
            .client
            .call::<CurrentUserGrants>(current_user_grants::Variables)
            .await?;
        Ok(CurrentUserGrantsResponse(response_data))
    }

    pub(crate) async fn principal_grants(
        &self,
        org_id: OrgId,
        principal: Principal,
    ) -> Result<PrincipalGrantsResponse> {
        let variables = principal_grants::Variables {
            input: principal_grants::IamPrincipalGrantsInput { org_id, principal },
        };
        let response_data = self.client.call::<PrincipalGrants>(variables).await?;
        Ok(PrincipalGrantsResponse(response_data))
    }

    pub(crate) async fn list_roles(&self, org_id: OrgId) -> Result<ListRolesResponse> {
        let variables = list_roles::Variables {
            input: list_roles::IamListRolesInput { org_id },
        };
        let response_data = self.client.call::<ListRoles>(variables).await?;
        Ok(ListRolesResponse(response_data))
    }

    pub(crate) async fn get_role(&self, role_urn: Urn) -> Result<GetRoleResponse> {
        let variables = get_role::Variables { role_urn };
        let response_data = self.client.call::<GetRole>(variables).await?;
        Ok(GetRoleResponse(response_data))
    }

    pub(crate) async fn permissions(&self) -> Result<PermissionsResponse> {
        let response_data = self
            .client
            .call::<Permissions>(permissions::Variables)
            .await?;
        Ok(PermissionsResponse(response_data))
    }

    pub(crate) async fn has_permission(
        &self,
        on_resource_urn: Urn,
        principal: Principal,
        permission_key: PermissionKey,
    ) -> Result<HasPermissionResponse> {
        let variables = has_permission::Variables {
            input: has_permission::IamHasPermissionInput {
                on_resource_urn,
                principal,
                permission_key,
            },
        };
        let response_data = self.client.call::<HasPermission>(variables).await?;
        Ok(HasPermissionResponse(response_data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OutputFormat;
    use crate::output::{render, Context};
    use serde_json::json;

    fn table(output: &dyn SerializableOutput) -> (String, String) {
        let cx = Context {
            width: None,
            now: "2026-11-03T12:00:00Z".parse().unwrap(),
        };
        let printed = render(OutputFormat::Table, output, &cx).unwrap();
        let strip = |s: &str| anstream::adapter::strip_str(s).to_string();
        (strip(&printed.out), strip(&printed.err))
    }

    #[test]
    fn members_are_listed_by_their_email() {
        let data: org_members::ResponseData = serde_json::from_value(json!({"iam": {"orgMembers": [
            {"principal": "user:u1", "roleUrns": ["urn:origin:cloud:iam::acme::role/owner"],
             "user": {"userId": "u1", "firstName": "Ada", "lastName": "Lovelace",
                      "email": "ada@example.com", "enabled": true}},
            {"principal": "user:u2", "roleUrns": ["urn:origin:cloud:iam::acme::role/viewer", "urn:origin:cloud:iam::acme::role/billing"],
             "user": {"userId": "u2", "firstName": "Alan", "lastName": "Turing",
                      "email": "alan@example.com", "enabled": false}},
        ]}}))
        .unwrap();
        assert_eq!(
            table(&OrgMembersResponse(data)).0,
            "\
EMAIL              FIRST NAME   LAST NAME   ROLES             ENABLED
ada@example.com    Ada          Lovelace    owner             true
alan@example.com   Alan         Turing      viewer, billing   false
"
        );
    }

    #[test]
    fn invites_and_service_accounts_say_how_long_ago() {
        let data: org_invites::ResponseData = serde_json::from_value(json!({"iam": {"orgInvites": [
            {"id": "i1", "orgId": "o1", "email": "grace@example.com", "roleUrn": "urn:origin:cloud:iam::acme::role/viewer",
             "invitedBy": "user:u1", "createdAt": "2026-11-01T12:00:00Z"},
        ]}}))
        .unwrap();
        assert_eq!(
            table(&OrgInvitesResponse(data)).0,
            "\
EMAIL               ROLE     INVITED BY   CREATED AT
grace@example.com   viewer   user:u1      2d ago
"
        );

        let none: service_accounts::ResponseData =
            serde_json::from_value(json!({"iam": {"serviceAccounts": []}})).unwrap();
        assert_eq!(
            table(&ServiceAccountsResponse(none)).1,
            "No service accounts. Create one with `origin iam create-service-account`.\n"
        );
    }

    #[test]
    fn a_grant_shows_its_roles() {
        let data: principal_grants::ResponseData = serde_json::from_value(json!({"iam": {"principalGrants": [
            {"id": "g1", "urn": "urn:origin:cloud:iam::acme::grant/g1", "principal": "user:u1", "orgId": "o1", "projectId": "p1",
             "roles": [{"roleUrn": "urn:origin:cloud:iam::acme::role/viewer"}],
             "createdAt": "2026-11-03T11:00:00Z", "createdBy": "user:u0"},
        ]}}))
        .unwrap();
        assert_eq!(
            table(&PrincipalGrantsResponse(data)).0,
            "\
PRINCIPAL   PROJECT ID   ROLE     CREATED AT
user:u1     p1           viewer   60m ago
"
        );
    }
}
