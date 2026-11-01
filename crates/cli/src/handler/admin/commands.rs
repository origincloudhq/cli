use crate::confirm::Confirmation;
use crate::identity::{OrgId, ProjectId};
use clap::Subcommand;

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum AdministrationCommands {
    /// Create organisation from its name alone; its billing details are
    /// added afterwards on its default billing account. register-organisation
    /// does both in one step.
    CreateOrganisation {
        /// Name of the organization.
        #[arg(short, long)]
        name: String,
    },

    /// Register organisation. The billing details become its default billing
    /// account, so the organisation is billable from the start.
    RegisterOrganisation {
        /// Name of the organization.
        #[arg(short, long)]
        name: String,
        /// Street and number of the billing address.
        #[arg(long)]
        street: String,
        /// City of the billing address.
        #[arg(long)]
        city: String,
        /// Postal code of the billing address.
        #[arg(long)]
        postal_code: String,
        /// Country of the billing address.
        #[arg(long)]
        country: String,
        /// Email address invoices are sent to.
        #[arg(long)]
        invoice_email: String,
        /// Person invoices are addressed to.
        #[arg(long)]
        contact_person: String,
        /// VAT identification number, if any.
        #[arg(long)]
        vat_id: Option<String>,
    },

    /// List accessible organisations.
    ListOrganisations {},

    /// Get an organization by its id or URN.
    GetOrganisation {
        /// Id of the organisation.
        #[arg(long)]
        id: OrgId,
    },
    /// Create a project in an organization.
    CreateProject {
        /// Name of the project.
        #[arg(long)]
        name: String,
        /// Prefix of the project id.
        #[arg(long)]
        id_prefix: Option<String>,
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,
    },

    /// List accessible projects.
    ListProjects {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,
    },

    /// Get a project by its id or URN.
    GetProject {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,
        /// Id of the project. Use with org-id.
        #[arg(long)]
        id: ProjectId,
    },

    /// Rename an organisation.
    UpdateOrganisation {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,
        /// New name.
        #[arg(short, long)]
        name: String,
    },

    /// Delete an organisation. Refuses unless it has zero projects.
    DeleteOrganisation {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// Rename a project.
    UpdateProject {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,
        /// Id of the project.
        #[arg(long)]
        id: ProjectId,
        /// New name.
        #[arg(short, long)]
        name: String,
    },

    /// Delete a project. Refuses while it still owns any resource
    /// (instance, image, key pair, volume, snapshot, NAT gateway, Floating
    /// IP, VPC, DNS policy other than the default) and names each kind
    /// left. The default DNS policy is deleted with the project.
    DeleteProject {
        /// ID of the organisation.
        #[arg(long)]
        org_id: OrgId,
        /// Id of the project.
        #[arg(long)]
        id: ProjectId,

        #[command(flatten)]
        confirmation: Confirmation,
    },
}
