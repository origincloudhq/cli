use super::commands::StorageCommands;
use crate::output::SerializableOutput;
use crate::service_apis::storage::StorageServiceClient;
use url::Url;

pub async fn run_storage_command(
    api_url: &Url,
    command: StorageCommands,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let client = StorageServiceClient::new(api_url.clone());

    match command {
        StorageCommands::SetResourceTags { resource_urn, tags } => Ok(Box::new(
            client.set_resource_tags(resource_urn, tags).await?,
        )),
        StorageCommands::RemoveResourceTags { resource_urn, keys } => Ok(Box::new(
            client.remove_resource_tags(resource_urn, keys).await?,
        )),
    }
}
