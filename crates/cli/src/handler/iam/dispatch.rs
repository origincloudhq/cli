use super::commands::IamCommands;
use crate::config::ClientProfile;
use crate::output::SerializableOutput;
use crate::service_apis::iam::IamServiceClient;
use url::Url;

pub async fn run_iam_command(
    _profile: &ClientProfile,
    api_url: &Url,
    command: IamCommands,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let client = IamServiceClient::new(api_url.clone());

    match command {
        IamCommands::InviteMember {
            org_id,
            email,
            role_urn,
        } => Ok(Box::new(
            client.invite_member(org_id, email, role_urn).await?,
        )),
        IamCommands::RemoveMember { org_id, principal } => {
            Ok(Box::new(client.remove_member(org_id, principal).await?))
        }
        IamCommands::RevokeInvite { org_id, invite_id } => {
            Ok(Box::new(client.revoke_invite(org_id, invite_id).await?))
        }
        IamCommands::ListMembers { org_id } => Ok(Box::new(client.org_members(org_id).await?)),
        IamCommands::ListInvites { org_id } => Ok(Box::new(client.org_invites(org_id).await?)),
        IamCommands::CreateServiceAccount { org_id, name } => {
            Ok(Box::new(client.create_service_account(org_id, name).await?))
        }
        IamCommands::RevokeServiceAccount {
            org_id,
            service_account_id,
            confirmation,
        } => Ok(Box::new(
            client
                .revoke_service_account(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "revoke",
                            what: "service account",
                            target: &service_account_id.clone(),
                            consequence: "Its credential stops working for good.",
                        },
                    )?,
                    org_id,
                    service_account_id,
                )
                .await?,
        )),
        IamCommands::ListServiceAccounts { org_id } => {
            Ok(Box::new(client.service_accounts(org_id).await?))
        }
        IamCommands::AssignRole {
            org_id,
            project_id,
            principal,
            role_urn,
        } => Ok(Box::new(
            client
                .assign_role(org_id, project_id, principal, role_urn)
                .await?,
        )),
        IamCommands::RevokeRole {
            org_id,
            project_id,
            principal,
            role_urn,
        } => Ok(Box::new(
            client
                .revoke_role(org_id, project_id, principal, role_urn)
                .await?,
        )),
        IamCommands::CurrentUser {} => Ok(Box::new(client.current_user().await?)),
        IamCommands::CurrentUserGrants {} => Ok(Box::new(client.current_user_grants().await?)),
        IamCommands::PrincipalGrants { org_id, principal } => {
            Ok(Box::new(client.principal_grants(org_id, principal).await?))
        }
        IamCommands::ListRoles { org_id } => Ok(Box::new(client.list_roles(org_id).await?)),
        IamCommands::GetRole { role_urn } => Ok(Box::new(client.get_role(role_urn).await?)),
        IamCommands::Permissions {} => Ok(Box::new(client.permissions().await?)),
        IamCommands::HasPermission {
            on_resource_urn,
            principal,
            permission_key,
        } => Ok(Box::new(
            client
                .has_permission(on_resource_urn, principal, permission_key)
                .await?,
        )),
    }
}
