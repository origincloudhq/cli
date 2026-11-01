use crate::client_auth::ConfigAuthenticator;
use crate::graphql::Error as ClientError;
use crate::graphql::{GenericGraphQLClient, Retries};
use crate::handler::tag_args::TagArg;
#[allow(unused_imports)]
use crate::identity::*;
use crate::output::{SerializableOutput, SerializationError};
use graphql_client::*;
use url::Url;

pub type Result<T> = std::result::Result<T, ClientError>;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/storage/mutation_set_resource_tags.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct SetStorageResourceTags;

#[derive(serde::Serialize)]
pub(crate) struct SetStorageResourceTagsResponse(
    crate::service_apis::storage::set_storage_resource_tags::ResponseData,
);

impl SerializableOutput for crate::service_apis::storage::SetStorageResourceTagsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/storage/mutation_remove_resource_tags.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct RemoveStorageResourceTags;

#[derive(serde::Serialize)]
pub(crate) struct RemoveStorageResourceTagsResponse(
    crate::service_apis::storage::remove_storage_resource_tags::ResponseData,
);

impl SerializableOutput for crate::service_apis::storage::RemoveStorageResourceTagsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

pub(crate) struct StorageServiceClient {
    client: GenericGraphQLClient<ConfigAuthenticator>,
}

impl StorageServiceClient {
    pub(crate) fn new(api_url: Url) -> Self {
        Self {
            client: GenericGraphQLClient::new(api_url, ConfigAuthenticator::default())
                .retrying(Retries::default()),
        }
    }

    pub(crate) async fn set_resource_tags(
        &self,
        resource_urn: Urn,
        tags: Vec<TagArg>,
    ) -> Result<SetStorageResourceTagsResponse> {
        let variables = set_storage_resource_tags::Variables {
            input: set_storage_resource_tags::PvsSetResourceTagsInput {
                resource_urn,
                tags: tags
                    .into_iter()
                    .map(|tag| set_storage_resource_tags::ResourceTagInput {
                        key: tag.key,
                        value: tag.value,
                    })
                    .collect(),
            },
        };
        let response_data = self
            .client
            .call::<SetStorageResourceTags>(variables)
            .await?;
        Ok(SetStorageResourceTagsResponse(response_data))
    }

    pub(crate) async fn remove_resource_tags(
        &self,
        resource_urn: Urn,
        keys: Vec<String>,
    ) -> Result<RemoveStorageResourceTagsResponse> {
        let variables = remove_storage_resource_tags::Variables {
            input: remove_storage_resource_tags::PvsRemoveResourceTagsInput { resource_urn, keys },
        };
        let response_data = self
            .client
            .call::<RemoveStorageResourceTags>(variables)
            .await?;
        Ok(RemoveStorageResourceTagsResponse(response_data))
    }
}
