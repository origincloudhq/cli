use super::addresses::{project_of, with_addresses};
use super::commands::ComputeCommands;
use crate::config::ClientProfile;
use crate::output::SerializableOutput;
use crate::service_apis::compute::{ComputeServiceClient, CreateImageParams, LaunchInstanceParams};
use url::Url;

pub async fn run_compute_command(
    _profile: &ClientProfile,
    api_url: &Url,
    command: ComputeCommands,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let client = ComputeServiceClient::new(api_url.clone());
    match command {
        ComputeCommands::SetResourceTags { resource_urn, tags } => Ok(Box::new(
            client.set_resource_tags(resource_urn, tags).await?,
        )),
        ComputeCommands::RemoveResourceTags { resource_urn, keys } => Ok(Box::new(
            client.remove_resource_tags(resource_urn, keys).await?,
        )),
        ComputeCommands::CreateImage {
            project_urn,
            root_device_name,
            root_device_size,
            root_device_snapshot_urn,
            description,
            default_user,
            tags,
        } => Ok(Box::new(
            client
                .create_image(CreateImageParams {
                    project_urn,
                    root_device_name,
                    root_device_size,
                    root_device_snapshot_urn,
                    description,
                    default_user,
                    tags,
                })
                .await?,
        )),
        ComputeCommands::ListImages { project_urn } => {
            Ok(Box::new(client.list_images(project_urn).await?))
        }
        ComputeCommands::GetImage { image_urn } => Ok(Box::new(client.get_image(image_urn).await?)),
        ComputeCommands::UpdateImage {
            image_urn,
            description,
        } => Ok(Box::new(client.update_image(image_urn, description).await?)),
        ComputeCommands::DeleteImage {
            image_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_image(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "image",
                            target: &image_urn.to_string(),
                            consequence: "It can't be restored.",
                        },
                    )?,
                    image_urn,
                )
                .await?,
        )),
        ComputeCommands::LaunchInstance {
            project_urn,
            image_urn,
            image_family,
            subnet_urn,
            key_pair_urn,
            instance_type,
            volume_backend,
            boot_volume_size_gib,
            boot_volume_storage_class,
            security_group_urn,
            login_user,
            tags,
        } => Ok(Box::new(
            client
                .launch_instance(LaunchInstanceParams {
                    project_urn,
                    image_urn,
                    image_family,
                    subnet_urn,
                    key_pair_urn,
                    instance_type,
                    volume_backend,
                    boot_volume_size_gib,
                    boot_volume_storage_class,
                    security_group_urns: security_group_urn,
                    login_user,
                    tags,
                })
                .await?,
        )),
        ComputeCommands::ListInstanceTypes => Ok(Box::new(client.list_instance_types().await?)),
        ComputeCommands::ListImageFamilies => Ok(Box::new(client.list_image_families().await?)),
        ComputeCommands::ImportKeyPair {
            project_urn,
            name,
            public_key,
        } => Ok(Box::new(
            client
                .import_key_pair(project_urn, name, public_key)
                .await?,
        )),
        ComputeCommands::GenerateKeyPair { project_urn, name } => {
            Ok(Box::new(client.generate_key_pair(project_urn, name).await?))
        }
        ComputeCommands::ListKeyPairs { project_urn } => {
            Ok(Box::new(client.list_key_pairs(project_urn).await?))
        }
        ComputeCommands::GetKeyPair { key_pair_urn } => {
            Ok(Box::new(client.get_key_pair(key_pair_urn).await?))
        }
        ComputeCommands::DeleteKeyPair {
            key_pair_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .delete_key_pair(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "delete",
                            what: "key pair",
                            target: &key_pair_urn.to_string(),
                            consequence: "It can't be restored.",
                        },
                    )?,
                    key_pair_urn,
                )
                .await?,
        )),
        ComputeCommands::ListInstances { project_urn } => {
            let instances = client.list_instances(project_urn.clone()).await?;
            Ok(Box::new(
                with_addresses(api_url, project_urn, json(&instances)?).await?,
            ))
        }
        ComputeCommands::GetInstance { instance_urn } => {
            let project_urn = project_of(&instance_urn)?;
            let instance = client.get_instance(instance_urn).await?;
            Ok(Box::new(
                with_addresses(api_url, project_urn, json(&instance)?).await?,
            ))
        }
        ComputeCommands::ListVolumeAttachments {
            project_urn,
            volume_urn,
        } => Ok(Box::new(
            client
                .list_volume_attachments(project_urn, volume_urn)
                .await?,
        )),
        ComputeCommands::StartInstance { instance_urn } => {
            Ok(Box::new(client.start_instance(instance_urn).await?))
        }
        ComputeCommands::ChangeInstanceType {
            instance_urn,
            instance_type,
        } => Ok(Box::new(
            client
                .change_instance_type(instance_urn, instance_type)
                .await?,
        )),
        ComputeCommands::StopInstance { instance_urn } => {
            Ok(Box::new(client.stop_instance(instance_urn).await?))
        }
        ComputeCommands::TerminateInstance {
            instance_urn,
            confirmation,
        } => Ok(Box::new(
            client
                .terminate_instance(
                    crate::confirm::require(
                        confirmation,
                        crate::confirm::Destructive {
                            verb: "terminate",
                            what: "instance",
                            target: &instance_urn.to_string(),
                            consequence: "It can't be started again.",
                        },
                    )?,
                    instance_urn,
                )
                .await?,
        )),
        ComputeCommands::AttachVolume {
            instance_urn,
            volume_urn,
            device_name,
        } => Ok(Box::new(
            client
                .attach_volume(instance_urn, volume_urn, device_name)
                .await?,
        )),
        ComputeCommands::DetachVolume {
            instance_urn,
            volume_urn,
        } => Ok(Box::new(
            client.detach_volume(instance_urn, volume_urn).await?,
        )),
    }
}

fn json(output: &dyn SerializableOutput) -> anyhow::Result<serde_json::Value> {
    output.to_json().map_err(|err| anyhow::anyhow!("{err}"))
}
