use super::commands::AdministrationCommands;
use crate::config::ClientProfile;
use crate::output::SerializableOutput;
use crate::service_apis::administration::{AdministrationServiceClient, NewOrganisation};
use url::Url;

pub async fn run_admin_command(
    _profile: &ClientProfile,
    api_url: &Url,
    cmd: AdministrationCommands,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let client = AdministrationServiceClient::new(api_url.clone());
    match cmd {
        AdministrationCommands::CreateOrganisation { name } => {
            Ok(Box::new(client.create_organisation(name).await?))
        }
        AdministrationCommands::RegisterOrganisation {
            name,
            street,
            city,
            postal_code,
            country,
            invoice_email,
            contact_person,
            vat_id,
        } => Ok(Box::new(
            client
                .register_organisation(NewOrganisation {
                    name,
                    street,
                    city,
                    postal_code,
                    country,
                    invoice_email,
                    contact_person,
                    vat_id,
                })
                .await?,
        )),
        AdministrationCommands::ListOrganisations { .. } => {
            Ok(Box::new(client.list_organisations().await?))
        }
        AdministrationCommands::GetOrganisation { id } => {
            Ok(Box::new(client.get_organisation_by_id(id).await?))
        }
        AdministrationCommands::CreateProject {
            id_prefix,
            name,
            org_id,
        } => Ok(Box::new(
            client.create_project(org_id, id_prefix, name).await?,
        )),
        AdministrationCommands::ListProjects { org_id } => {
            Ok(Box::new(client.list_projects(org_id).await?))
        }
        AdministrationCommands::GetProject { id, org_id } => {
            Ok(Box::new(client.get_project_by_id(org_id, id).await?))
        }
        AdministrationCommands::UpdateOrganisation { org_id, name } => {
            Ok(Box::new(client.update_organisation(org_id, name).await?))
        }
        AdministrationCommands::DeleteOrganisation {
            org_id,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_organisation(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "organisation",
                            target: &org_id.to_string(),
                            consequence: "It can't be restored.",
                        },
                    )?,
                    org_id,
                )
                .await?,
        )),
        AdministrationCommands::UpdateProject { org_id, id, name } => {
            Ok(Box::new(client.update_project(org_id, id, name).await?))
        }
        AdministrationCommands::DeleteProject {
            org_id,
            id,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_project(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "project",
                            target: &format!("{org_id}/{id}"),
                            consequence: "It can't be restored.",
                        },
                    )?,
                    org_id,
                    id,
                )
                .await?,
        )),
    }
}
