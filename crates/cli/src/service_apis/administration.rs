use crate::client_auth::ConfigAuthenticator;
use crate::graphql::Error as ClientError;
use crate::graphql::GenericGraphQLClient;
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
    query_path = "graphql/administration/mutation_create_organisation.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CreateOrganisation;

#[derive(serde::Serialize)]
pub(crate) struct CreateOrganisationResponse(create_organisation::ResponseData);

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/administration/mutation_register_organisation.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct RegisterOrganisation;

#[derive(serde::Serialize)]
pub(crate) struct RegisterOrganisationResponse(register_organisation::ResponseData);

impl SerializableOutput for RegisterOrganisationResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

/// What `register-organisation` collects: the name plus the billing profile
/// the organisation's default billing account is created with.
/// Flat on purpose - it mirrors the command's flags one-to-one, and the
/// nesting into the wire's address object happens here, at the boundary.
pub(crate) struct NewOrganisation {
    pub(crate) name: String,
    pub(crate) street: String,
    pub(crate) city: String,
    pub(crate) postal_code: String,
    pub(crate) country: String,
    pub(crate) invoice_email: String,
    pub(crate) contact_person: String,
    pub(crate) vat_id: Option<String>,
}

impl SerializableOutput for CreateOrganisationResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/administration/query_get_organisation_by_id.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct OrganisationById;

#[derive(serde::Serialize)]
pub(crate) struct GetOrganisationByIdResponse(organisation_by_id::ResponseData);

impl SerializableOutput for GetOrganisationByIdResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/administration/query_list_organisations.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct Organisations;

#[derive(serde::Serialize)]
pub(crate) struct ListOrganisationsResponse(organisations::ResponseData);

impl SerializableOutput for ListOrganisationsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/administration/mutation_create_project.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CreateProject;

#[derive(serde::Serialize)]
pub(crate) struct CreateProjectResponse(create_project::ResponseData);

impl SerializableOutput for CreateProjectResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/administration/mutation_update_organisation.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct UpdateOrganisation;

#[derive(serde::Serialize)]
pub(crate) struct UpdateOrganisationResponse(update_organisation::ResponseData);

impl SerializableOutput for UpdateOrganisationResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/administration/mutation_delete_organisation.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DeleteOrganisation;

#[derive(serde::Serialize)]
pub(crate) struct DeleteOrganisationResponse(delete_organisation::ResponseData);

impl SerializableOutput for DeleteOrganisationResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/administration/query_get_project_by_id.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ProjectById;

#[derive(serde::Serialize)]
pub(crate) struct GetProjectByIdResponse(project_by_id::ResponseData);

impl SerializableOutput for GetProjectByIdResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/administration/mutation_update_project.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct UpdateProject;

#[derive(serde::Serialize)]
pub(crate) struct UpdateProjectResponse(update_project::ResponseData);

impl SerializableOutput for UpdateProjectResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/administration/mutation_delete_project.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DeleteProject;

#[derive(serde::Serialize)]
pub(crate) struct DeleteProjectResponse(delete_project::ResponseData);

impl SerializableOutput for DeleteProjectResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/administration/query_list_projects.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct Projects;

#[derive(serde::Serialize)]
pub(crate) struct ListProjectsResponse(projects::ResponseData);

impl SerializableOutput for ListProjectsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

pub(crate) struct AdministrationServiceClient {
    client: GenericGraphQLClient<ConfigAuthenticator>,
}

impl AdministrationServiceClient {
    pub(crate) fn new(api_url: Url) -> Self {
        Self {
            client: GenericGraphQLClient::new(api_url, ConfigAuthenticator::default()),
        }
    }

    pub(crate) async fn create_organisation(
        &self,
        name: String,
    ) -> Result<CreateOrganisationResponse> {
        let variables = create_organisation::Variables {
            input: create_organisation::AdminCreateOrganisationInput { name },
        };
        let response_data = self.client.call::<CreateOrganisation>(variables).await?;
        Ok(CreateOrganisationResponse(response_data))
    }

    pub(crate) async fn register_organisation(
        &self,
        new_organisation: NewOrganisation,
    ) -> Result<RegisterOrganisationResponse> {
        let variables = register_organisation::Variables {
            input: register_organisation::AdminRegisterOrganisationInput {
                name: new_organisation.name,
                billing_address: register_organisation::BillingAddressInput {
                    street: new_organisation.street,
                    city: new_organisation.city,
                    postal_code: new_organisation.postal_code,
                    country: new_organisation.country,
                },
                invoice_email: new_organisation.invoice_email,
                vat_id: new_organisation.vat_id,
                contact_person: new_organisation.contact_person,
            },
        };
        let response_data = self.client.call::<RegisterOrganisation>(variables).await?;
        Ok(RegisterOrganisationResponse(response_data))
    }

    pub(crate) async fn list_organisations(&self) -> Result<ListOrganisationsResponse> {
        let response_data = self
            .client
            .call::<Organisations>(organisations::Variables)
            .await?;
        Ok(ListOrganisationsResponse(response_data))
    }

    pub(crate) async fn get_organisation_by_id(
        &self,
        org_id: OrgId,
    ) -> Result<GetOrganisationByIdResponse> {
        let variables = organisation_by_id::Variables { org_id };
        let response_data = self.client.call::<OrganisationById>(variables).await?;
        Ok(GetOrganisationByIdResponse(response_data))
    }

    pub(crate) async fn create_project(
        &self,
        org_id: OrgId,
        id_prefix: Option<String>,
        name: String,
    ) -> Result<CreateProjectResponse> {
        let variables = create_project::Variables {
            input: create_project::AdminCreateProjectInput {
                org_id,
                id_prefix,
                name,
            },
        };
        let response_data = self.client.call::<CreateProject>(variables).await?;
        Ok(CreateProjectResponse(response_data))
    }

    pub(crate) async fn list_projects(&self, org_id: OrgId) -> Result<ListProjectsResponse> {
        let variables = projects::Variables {
            input: projects::AdminListProjectsInput { org_id },
        };
        let response_data = self.client.call::<Projects>(variables).await?;
        Ok(ListProjectsResponse(response_data))
    }

    pub(crate) async fn get_project_by_id(
        &self,
        org_id: OrgId,
        project_id: ProjectId,
    ) -> Result<GetProjectByIdResponse> {
        let variables = project_by_id::Variables { org_id, project_id };
        let response_data = self.client.call::<ProjectById>(variables).await?;
        Ok(GetProjectByIdResponse(response_data))
    }

    pub(crate) async fn update_organisation(
        &self,
        org_id: OrgId,
        name: String,
    ) -> Result<UpdateOrganisationResponse> {
        let variables = update_organisation::Variables {
            input: update_organisation::AdminUpdateOrganisationInput { org_id, name },
        };
        let response_data = self.client.call::<UpdateOrganisation>(variables).await?;
        Ok(UpdateOrganisationResponse(response_data))
    }

    pub(crate) async fn delete_organisation(
        &self,
        _confirmed: crate::confirm::Confirmed,
        org_id: OrgId,
    ) -> Result<DeleteOrganisationResponse> {
        let variables = delete_organisation::Variables {
            input: delete_organisation::AdminDeleteOrganisationInput { org_id },
        };
        let response_data = self.client.call::<DeleteOrganisation>(variables).await?;
        Ok(DeleteOrganisationResponse(response_data))
    }

    pub(crate) async fn update_project(
        &self,
        org_id: OrgId,
        project_id: ProjectId,
        name: String,
    ) -> Result<UpdateProjectResponse> {
        let variables = update_project::Variables {
            input: update_project::AdminUpdateProjectInput {
                org_id,
                project_id,
                name,
            },
        };
        let response_data = self.client.call::<UpdateProject>(variables).await?;
        Ok(UpdateProjectResponse(response_data))
    }

    pub(crate) async fn delete_project(
        &self,
        _confirmed: crate::confirm::Confirmed,
        org_id: OrgId,
        project_id: ProjectId,
    ) -> Result<DeleteProjectResponse> {
        let variables = delete_project::Variables {
            input: delete_project::AdminDeleteProjectInput { org_id, project_id },
        };
        let response_data = self.client.call::<DeleteProject>(variables).await?;
        Ok(DeleteProjectResponse(response_data))
    }
}
