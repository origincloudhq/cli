use crate::confirm::Confirmation;
use crate::identity::Principal;
use crate::identity::Urn;
use crate::identity::{OrgId, ProjectId};
use crate::service_apis::iam::PermissionKey;
use clap::Subcommand;

/// Covers the IAM operations that are real and usable today: org member
/// management (invite/list/remove), service accounts / API keys
/// (create/revoke/list), role assignment against built-in roles,
/// current-user/grants introspection, and permission checks. Group-scoped
/// role assignment, custom (org-defined)
/// roles, and role-assignment conditions are all still
/// stubbed at the API level - `roles`/`get-role` only ever return
/// built-in roles as a result, and there is deliberately no group-scoped
/// or conditional variant of `assign-role`/`revoke-role` here, to avoid
/// silently accepting input the backend would ignore.
#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum IamCommands {
    /// Invite a person to an org by email. Grants access immediately if
    /// they already have an account; otherwise stores a pending invite
    /// applied automatically the first time they authenticate.
    InviteMember {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,

        /// Email address to invite.
        #[arg(long)]
        email: String,

        /// URN of the built-in role to grant once accepted.
        #[arg(long)]
        role_urn: Urn,
    },

    /// Remove a member's access to an org - revokes every role they hold
    /// in it, at every project scope.
    RemoveMember {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,

        /// The principal to remove, e.g. `user:<user-id>`.
        #[arg(long)]
        principal: Principal,
    },

    /// Cancel a pending invite before it's been accepted.
    RevokeInvite {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,

        /// ID of the invite to revoke (see `list-invites`).
        #[arg(long)]
        invite_id: String,
    },

    /// List every principal with an active role assignment on an org.
    ListMembers {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,
    },

    /// List pending (not yet accepted) invites for an org.
    ListInvites {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,
    },

    /// Create a service account for an organization. The response contains an
    /// OAuth client ID and client secret for the client-credentials grant;
    /// the secret is shown once, so store it now. The service account starts
    /// with no roles; grant it access with `assign-role`.
    CreateServiceAccount {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,

        /// Display name for the service account.
        #[arg(long)]
        name: String,
    },

    /// Revoke a service account. Its credential stops working immediately,
    /// and every role assignment it holds is removed, so an access token
    /// issued before the revocation is refused too. A revoked service
    /// account can't be reactivated or assigned a role again.
    RevokeServiceAccount {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,

        /// ID of the service account to revoke (see `list-service-accounts`).
        #[arg(long)]
        service_account_id: String,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// List every service account (active or revoked) created for an org.
    ListServiceAccounts {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,
    },

    /// Assign a built-in role to a principal, org-wide or project-scoped.
    AssignRole {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,

        /// Narrows the assignment to a single project. Omit for an
        /// org-wide grant.
        #[arg(long)]
        project_id: Option<ProjectId>,

        /// The principal to grant the role to, e.g. `user:<user-id>`.
        #[arg(long)]
        principal: Principal,

        /// URN of the built-in role to assign.
        #[arg(long)]
        role_urn: Urn,
    },

    /// Revoke a previously assigned role from a principal.
    RevokeRole {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,

        /// Must match the scope (org-wide vs. the given project) the role
        /// was originally assigned with.
        #[arg(long)]
        project_id: Option<ProjectId>,

        /// The principal to revoke the role from, e.g. `user:<user-id>`.
        #[arg(long)]
        principal: Principal,

        /// URN of the role to revoke.
        #[arg(long)]
        role_urn: Urn,
    },

    /// Show the currently authenticated user.
    CurrentUser {},

    /// List the currently authenticated user's role grants.
    CurrentUserGrants {},

    /// List every role a principal holds in an org, with each grant's
    /// project scope.
    PrincipalGrants {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,

        /// The principal to look up, e.g. `user:<user-id>`.
        #[arg(long)]
        principal: Principal,
    },

    /// List the built-in roles available in an organization.
    ListRoles {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,
    },

    /// Get a single built-in role and its permissions.
    GetRole {
        /// URN of the role.
        #[arg(long)]
        role_urn: Urn,
    },

    /// List every permission the platform knows about.
    Permissions {},

    /// Check whether a principal has a given permission on a resource.
    HasPermission {
        /// URN of the resource to check the permission against.
        #[arg(long)]
        on_resource_urn: Urn,

        /// The principal to check, e.g. `user:<user-id>`.
        #[arg(long)]
        principal: Principal,

        /// Permission key to check, e.g. `network.vpc.list`.
        #[arg(long)]
        permission_key: PermissionKey,
    },
}
