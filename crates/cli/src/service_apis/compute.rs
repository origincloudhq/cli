use crate::client_auth::ConfigAuthenticator;
use crate::graphql::Error as ClientError;
use crate::graphql::{GenericGraphQLClient, Retries};
use crate::handler::tag_args::{tag_inputs, TagArg};
#[allow(unused_imports)]
use crate::identity::*;
use crate::output::{SerializableOutput, SerializationError};
use graphql_client::*;
use url::Url;

pub type Result<T> = std::result::Result<T, ClientError>;
/// The `DateTime` GraphQL scalar, as `graphql_client`'s codegen expects to
/// find it - same alias every other `service_apis` module declares.
pub type DateTime = chrono::DateTime<chrono::Utc>;

/// `ComputeServiceClient::launch_instance`'s params, bundled into a struct
/// to stay under clippy's `too_many_arguments` threshold now that
/// `instance_type` is required alongside the rest.
pub(crate) struct CreateImageParams {
    pub(crate) project_urn: Urn,
    pub(crate) root_device_name: String,
    pub(crate) root_device_size: i64,
    pub(crate) root_device_snapshot_urn: Urn,
    pub(crate) description: String,
    pub(crate) default_user: Option<String>,
    pub(crate) tags: Vec<TagArg>,
}

pub(crate) struct LaunchInstanceParams {
    pub(crate) project_urn: Urn,
    pub(crate) image_urn: Option<Urn>,
    pub(crate) image_family: Option<String>,
    pub(crate) subnet_urn: Option<Urn>,
    pub(crate) key_pair_urn: Urn,
    pub(crate) instance_type: String,
    pub(crate) volume_backend: Option<String>,
    pub(crate) boot_volume_size_gib: Option<u32>,
    pub(crate) boot_volume_storage_class: Option<String>,
    pub(crate) security_group_urns: Vec<Urn>,
    pub(crate) login_user: Option<String>,
    pub(crate) tags: Vec<TagArg>,
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_create_image.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct CreateImage;

#[derive(serde::Serialize)]
pub(crate) struct CreateImageResponse(create_image::ResponseData);

impl SerializableOutput for CreateImageResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/query_list_images.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListImages;

#[derive(serde::Serialize)]
pub(crate) struct ListImagesResponse(list_images::ResponseData);

impl SerializableOutput for ListImagesResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/query_get_image.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetImage;

#[derive(serde::Serialize)]
pub(crate) struct GetImageResponse(get_image::ResponseData);

impl SerializableOutput for GetImageResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_update_image.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct UpdateImage;

#[derive(serde::Serialize)]
pub(crate) struct UpdateImageResponse(update_image::ResponseData);

impl SerializableOutput for UpdateImageResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_delete_image.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DeleteImage;

#[derive(serde::Serialize)]
pub(crate) struct DeleteImageResponse(delete_image::ResponseData);

impl SerializableOutput for DeleteImageResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_import_key_pair.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ImportKeyPair;

#[derive(serde::Serialize)]
pub(crate) struct ImportKeyPairResponse(import_key_pair::ResponseData);

impl SerializableOutput for ImportKeyPairResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_generate_key_pair.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GenerateKeyPair;

#[derive(serde::Serialize)]
pub(crate) struct GenerateKeyPairResponse(generate_key_pair::ResponseData);

impl SerializableOutput for GenerateKeyPairResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/query_list_key_pairs.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListKeyPairs;

#[derive(serde::Serialize)]
pub(crate) struct ListKeyPairsResponse(list_key_pairs::ResponseData);

impl SerializableOutput for ListKeyPairsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/query_get_key_pair.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetKeyPair;

#[derive(serde::Serialize)]
pub(crate) struct GetKeyPairResponse(get_key_pair::ResponseData);

impl SerializableOutput for GetKeyPairResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_delete_key_pair.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct DeleteKeyPair;

#[derive(serde::Serialize)]
pub(crate) struct DeleteKeyPairResponse(delete_key_pair::ResponseData);

impl SerializableOutput for DeleteKeyPairResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/query_list_instance_types.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListInstanceTypes;

#[derive(serde::Serialize)]
pub(crate) struct ListInstanceTypesResponse(list_instance_types::ResponseData);

impl SerializableOutput for ListInstanceTypesResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/query_list_image_families.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListImageFamilies;

#[derive(serde::Serialize)]
pub(crate) struct ListImageFamiliesResponse(list_image_families::ResponseData);

impl SerializableOutput for ListImageFamiliesResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_launch_instance.graphql",
    response_derives = "Serialize"
)]
struct LaunchInstance;

#[derive(serde::Serialize)]
pub(crate) struct LaunchInstanceResponse(launch_instance::ResponseData);

impl SerializableOutput for LaunchInstanceResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_start_instance.graphql",
    response_derives = "Serialize"
)]
struct StartInstance;

#[derive(serde::Serialize)]
pub(crate) struct StartInstanceResponse(start_instance::ResponseData);

impl SerializableOutput for crate::service_apis::compute::StartInstanceResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_change_instance_type.graphql",
    response_derives = "Serialize"
)]
struct ChangeInstanceType;

#[derive(serde::Serialize)]
pub(crate) struct ChangeInstanceTypeResponse(change_instance_type::ResponseData);

impl SerializableOutput for ChangeInstanceTypeResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_stop_instance.graphql",
    response_derives = "Serialize"
)]
struct StopInstance;

#[derive(serde::Serialize)]
pub(crate) struct StopInstanceResponse(stop_instance::ResponseData);

impl SerializableOutput for StopInstanceResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_attach_volume.graphql",
    response_derives = "Serialize"
)]
struct AttachVolume;

#[derive(serde::Serialize)]
pub(crate) struct AttachVolumeResponse(attach_volume::ResponseData);

impl SerializableOutput for AttachVolumeResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_detach_volume.graphql",
    response_derives = "Serialize"
)]
struct DetachVolume;

#[derive(serde::Serialize)]
pub(crate) struct DetachVolumeResponse(detach_volume::ResponseData);

impl SerializableOutput for DetachVolumeResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_terminate_instance.graphql",
    response_derives = "Serialize"
)]
struct TerminateInstance;

#[derive(serde::Serialize)]
pub(crate) struct TerminateInstanceResponse(terminate_instance::ResponseData);

impl SerializableOutput for TerminateInstanceResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/query_get_instance.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct GetInstance;

#[derive(serde::Serialize)]
pub(crate) struct GetInstanceResponse(get_instance::ResponseData);

impl SerializableOutput for GetInstanceResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/query_list_instances.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListInstances;

#[derive(serde::Serialize)]
pub(crate) struct ListInstancesResponse(list_instances::ResponseData);

impl SerializableOutput for ListInstancesResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/query_list_volume_attachments.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct ListVolumeAttachments;

#[derive(serde::Serialize)]
pub(crate) struct ListVolumeAttachmentsResponse(list_volume_attachments::ResponseData);

impl SerializableOutput for ListVolumeAttachmentsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_set_resource_tags.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct SetComputeResourceTags;

#[derive(serde::Serialize)]
pub(crate) struct SetComputeResourceTagsResponse(
    crate::service_apis::compute::set_compute_resource_tags::ResponseData,
);

impl SerializableOutput for crate::service_apis::compute::SetComputeResourceTagsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/compute/mutation_remove_resource_tags.graphql",
    response_derives = "Serialize"
)]
pub(crate) struct RemoveComputeResourceTags;

#[derive(serde::Serialize)]
pub(crate) struct RemoveComputeResourceTagsResponse(
    crate::service_apis::compute::remove_compute_resource_tags::ResponseData,
);

impl SerializableOutput for crate::service_apis::compute::RemoveComputeResourceTagsResponse {
    fn to_json(&self) -> std::result::Result<serde_json::Value, SerializationError> {
        serde_json::to_value(self).map_err(|err| SerializationError::Other(err.to_string()))
    }
}

pub(crate) struct ComputeServiceClient {
    client: GenericGraphQLClient<ConfigAuthenticator>,
}

impl ComputeServiceClient {
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
    ) -> Result<SetComputeResourceTagsResponse> {
        let variables = set_compute_resource_tags::Variables {
            input: set_compute_resource_tags::ComputeSetResourceTagsInput {
                resource_urn,
                tags: tags
                    .into_iter()
                    .map(|tag| set_compute_resource_tags::ResourceTagInput {
                        key: tag.key,
                        value: tag.value,
                    })
                    .collect(),
            },
        };
        let response_data = self
            .client
            .call::<SetComputeResourceTags>(variables)
            .await?;
        Ok(SetComputeResourceTagsResponse(response_data))
    }

    pub(crate) async fn remove_resource_tags(
        &self,
        resource_urn: Urn,
        keys: Vec<String>,
    ) -> Result<RemoveComputeResourceTagsResponse> {
        let variables = remove_compute_resource_tags::Variables {
            input: remove_compute_resource_tags::ComputeRemoveResourceTagsInput {
                resource_urn,
                keys,
            },
        };
        let response_data = self
            .client
            .call::<RemoveComputeResourceTags>(variables)
            .await?;
        Ok(RemoveComputeResourceTagsResponse(response_data))
    }

    pub(crate) async fn create_image(
        &self,
        input: CreateImageParams,
    ) -> Result<CreateImageResponse> {
        let variables = create_image::Variables {
            input: create_image::ComputeCreateImageInput {
                project_urn: input.project_urn,
                description: input.description,
                default_user: input.default_user,
                root_device_name: input.root_device_name.clone(),
                block_device_specs: vec![create_image::ComputeBlockDeviceSpecInput {
                    device_name: input.root_device_name,
                    size_in_gib: input.root_device_size,
                    snapshot_urn: Some(input.root_device_snapshot_urn),
                }],
                tags: tag_inputs!(create_image, input.tags),
            },
        };
        let response_data = self.client.call::<CreateImage>(variables).await?;
        Ok(CreateImageResponse(response_data))
    }

    pub(crate) async fn list_images(&self, project_urn: Urn) -> Result<ListImagesResponse> {
        let variables = list_images::Variables {
            input: list_images::ComputeListImagesInput { project_urn },
        };
        let response_data = self.client.call::<ListImages>(variables).await?;
        Ok(ListImagesResponse(response_data))
    }

    pub(crate) async fn launch_instance(
        &self,
        input: LaunchInstanceParams,
    ) -> Result<LaunchInstanceResponse> {
        let variables = launch_instance::Variables {
            input: launch_instance::ComputeLaunchInstanceInput {
                project_urn: input.project_urn,
                image_urn: input.image_urn,
                image_family: input.image_family,
                subnet_urn: input.subnet_urn,
                key_pair_urn: input.key_pair_urn,
                instance_type: input.instance_type,
                volume_backend: input.volume_backend,
                // Omitted entirely when neither half was given, so a launch
                // that says nothing about its root disk keeps sending the
                // request it sent before these flags existed.
                boot_volume: (input.boot_volume_size_gib.is_some()
                    || input.boot_volume_storage_class.is_some())
                .then(|| launch_instance::ComputeBootVolumeInput {
                    size_in_gib: input.boot_volume_size_gib.map(i64::from),
                    storage_class: input.boot_volume_storage_class,
                }),
                security_group_urns: Some(input.security_group_urns),
                login_user: input.login_user,
                tags: tag_inputs!(launch_instance, input.tags),
            },
        };
        let response_data = self.client.call::<LaunchInstance>(variables).await?;
        Ok(LaunchInstanceResponse(response_data))
    }

    pub(crate) async fn list_instance_types(&self) -> Result<ListInstanceTypesResponse> {
        let variables = list_instance_types::Variables {};
        let response_data = self.client.call::<ListInstanceTypes>(variables).await?;
        Ok(ListInstanceTypesResponse(response_data))
    }

    pub(crate) async fn list_image_families(&self) -> Result<ListImageFamiliesResponse> {
        let variables = list_image_families::Variables {};
        let response_data = self.client.call::<ListImageFamilies>(variables).await?;
        Ok(ListImageFamiliesResponse(response_data))
    }

    pub(crate) async fn get_image(&self, image_urn: Urn) -> Result<GetImageResponse> {
        let variables = get_image::Variables { image_urn };
        let response_data = self.client.call::<GetImage>(variables).await?;
        Ok(GetImageResponse(response_data))
    }

    pub(crate) async fn update_image(
        &self,
        image_urn: Urn,
        description: String,
    ) -> Result<UpdateImageResponse> {
        let variables = update_image::Variables {
            input: update_image::ComputeUpdateImageInput {
                image_urn,
                description,
            },
        };
        let response_data = self.client.call::<UpdateImage>(variables).await?;
        Ok(UpdateImageResponse(response_data))
    }

    pub(crate) async fn delete_image(
        &self,
        _confirmed: crate::confirm::Confirmed,
        image_urn: Urn,
    ) -> Result<DeleteImageResponse> {
        let variables = delete_image::Variables {
            input: delete_image::ComputeDeleteImageInput { image_urn },
        };
        let response_data = self.client.call::<DeleteImage>(variables).await?;
        Ok(DeleteImageResponse(response_data))
    }

    pub(crate) async fn import_key_pair(
        &self,
        project_urn: Urn,
        name: String,
        public_key: String,
    ) -> Result<ImportKeyPairResponse> {
        let variables = import_key_pair::Variables {
            input: import_key_pair::ComputeImportKeyPairInput {
                project_urn,
                name,
                public_key,
            },
        };
        let response_data = self.client.call::<ImportKeyPair>(variables).await?;
        Ok(ImportKeyPairResponse(response_data))
    }

    pub(crate) async fn generate_key_pair(
        &self,
        project_urn: Urn,
        name: String,
    ) -> Result<GenerateKeyPairResponse> {
        let variables = generate_key_pair::Variables {
            input: generate_key_pair::ComputeGenerateKeyPairInput { project_urn, name },
        };
        let response_data = self.client.call::<GenerateKeyPair>(variables).await?;
        Ok(GenerateKeyPairResponse(response_data))
    }

    pub(crate) async fn list_key_pairs(&self, project_urn: Urn) -> Result<ListKeyPairsResponse> {
        let variables = list_key_pairs::Variables {
            input: list_key_pairs::ComputeListKeyPairsInput { project_urn },
        };
        let response_data = self.client.call::<ListKeyPairs>(variables).await?;
        Ok(ListKeyPairsResponse(response_data))
    }

    pub(crate) async fn get_key_pair(&self, key_pair_urn: Urn) -> Result<GetKeyPairResponse> {
        let variables = get_key_pair::Variables { key_pair_urn };
        let response_data = self.client.call::<GetKeyPair>(variables).await?;
        Ok(GetKeyPairResponse(response_data))
    }

    pub(crate) async fn delete_key_pair(
        &self,
        _confirmed: crate::confirm::Confirmed,
        key_pair_urn: Urn,
    ) -> Result<DeleteKeyPairResponse> {
        let variables = delete_key_pair::Variables {
            input: delete_key_pair::ComputeDeleteKeyPairInput { key_pair_urn },
        };
        let response_data = self.client.call::<DeleteKeyPair>(variables).await?;
        Ok(DeleteKeyPairResponse(response_data))
    }

    pub(crate) async fn start_instance(&self, instance_urn: Urn) -> Result<StartInstanceResponse> {
        let variables = start_instance::Variables {
            input: start_instance::ComputeStartInstanceInput { instance_urn },
        };
        let response_data = self.client.call::<StartInstance>(variables).await?;
        Ok(StartInstanceResponse(response_data))
    }

    pub(crate) async fn change_instance_type(
        &self,
        instance_urn: Urn,
        instance_type: String,
    ) -> Result<ChangeInstanceTypeResponse> {
        let variables = change_instance_type::Variables {
            input: change_instance_type::ComputeChangeInstanceTypeInput {
                instance_urn,
                instance_type,
            },
        };
        let response_data = self.client.call::<ChangeInstanceType>(variables).await?;
        Ok(ChangeInstanceTypeResponse(response_data))
    }

    pub(crate) async fn stop_instance(&self, instance_urn: Urn) -> Result<StopInstanceResponse> {
        let variables = stop_instance::Variables {
            input: stop_instance::ComputeStopInstanceInput { instance_urn },
        };
        let response_data = self.client.call::<StopInstance>(variables).await?;
        Ok(StopInstanceResponse(response_data))
    }

    pub(crate) async fn attach_volume(
        &self,
        instance_urn: Urn,
        volume_urn: Urn,
        device_name: Option<String>,
    ) -> Result<AttachVolumeResponse> {
        let variables = attach_volume::Variables {
            input: attach_volume::ComputeAttachVolumeInput {
                instance_urn,
                volume_urn,
                device_name,
            },
        };
        let response_data = self.client.call::<AttachVolume>(variables).await?;
        Ok(AttachVolumeResponse(response_data))
    }

    pub(crate) async fn detach_volume(
        &self,
        instance_urn: Urn,
        volume_urn: Urn,
    ) -> Result<DetachVolumeResponse> {
        let variables = detach_volume::Variables {
            input: detach_volume::ComputeDetachVolumeInput {
                instance_urn,
                volume_urn,
            },
        };
        let response_data = self.client.call::<DetachVolume>(variables).await?;
        Ok(DetachVolumeResponse(response_data))
    }

    pub(crate) async fn terminate_instance(
        &self,
        _confirmed: crate::confirm::Confirmed,
        instance_urn: Urn,
    ) -> Result<TerminateInstanceResponse> {
        let variables = terminate_instance::Variables {
            input: terminate_instance::ComputeTerminateInstanceInput { instance_urn },
        };
        let response_data = self.client.call::<TerminateInstance>(variables).await?;
        Ok(TerminateInstanceResponse(response_data))
    }

    pub(crate) async fn get_instance(&self, instance_urn: Urn) -> Result<GetInstanceResponse> {
        let variables = get_instance::Variables { instance_urn };
        let response_data = self.client.call::<GetInstance>(variables).await?;
        Ok(GetInstanceResponse(response_data))
    }

    /// Which attachments reference a volume, each naming its instance - the
    /// CLI half of "which instance holds this volume".
    pub(crate) async fn list_volume_attachments(
        &self,
        project_urn: Urn,
        volume_urn: Urn,
    ) -> Result<ListVolumeAttachmentsResponse> {
        let variables = list_volume_attachments::Variables {
            input: list_volume_attachments::ComputeListVolumeAttachmentsInput {
                project_urn,
                volume_urn,
            },
        };
        let response_data = self.client.call::<ListVolumeAttachments>(variables).await?;
        Ok(ListVolumeAttachmentsResponse(response_data))
    }

    pub(crate) async fn list_instances(&self, project_urn: Urn) -> Result<ListInstancesResponse> {
        let variables = list_instances::Variables {
            input: list_instances::ComputeListInstancesInput { project_urn },
        };
        let response_data = self.client.call::<ListInstances>(variables).await?;
        Ok(ListInstancesResponse(response_data))
    }
}
