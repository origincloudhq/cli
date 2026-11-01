//! Shows an instance's addresses next to its network interfaces.
//!
//! An instance names its interfaces (`networkInterfaceAttachments`) but its
//! addresses live in the network service: the interface carries the private
//! IPv4/IPv6 addresses and a Floating IP names the interface it is attached
//! to. Without this join a script that launches an instance cannot learn its
//! address from the instance, and has to guess it from `list-floating-ips`.
//!
//! Two project-wide queries (interfaces, Floating IPs) cover any number of
//! instances, so `list-instances` costs the same three requests as
//! `get-instance`.

use crate::identity::Urn;
use crate::output::{SerializableOutput, SerializationError};
use crate::service_apis::network::NetworkServiceClient;
use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::collections::HashMap;
use url::Url;

/// An instance query's output with each interface attachment's addresses
/// filled in.
pub(crate) struct InstancesWithAddresses(Value);

impl SerializableOutput for InstancesWithAddresses {
    fn to_json(&self) -> Result<Value, SerializationError> {
        Ok(self.0.clone())
    }
}

/// The project URN an instance URN belongs to.
pub(crate) fn project_of(instance_urn: &Urn) -> Result<Urn> {
    let org_id = instance_urn
        .org_id()
        .context("the instance URN names no organization")?;
    let project_id = instance_urn
        .project_id()
        .context("the instance URN names no project")?;
    Ok(Urn::of_project(&org_id, &project_id))
}

/// Fetches the project's interfaces and Floating IPs and adds the addresses
/// to every instance in `output`.
pub(crate) async fn with_addresses(
    api_url: &Url,
    project_urn: Urn,
    output: Value,
) -> Result<InstancesWithAddresses> {
    let network = NetworkServiceClient::new(api_url.clone());
    let interfaces = network
        .list_network_interfaces(project_urn.clone())
        .await
        .context("listing the project's network interfaces for the instance's addresses")?
        .to_json()
        .map_err(|err| anyhow::anyhow!("{err}"))?;
    let floating_ips = network
        .list_floating_ips(project_urn)
        .await
        .context("listing the project's Floating IPs for the instance's addresses")?
        .to_json()
        .map_err(|err| anyhow::anyhow!("{err}"))?;
    Ok(InstancesWithAddresses(join(
        output,
        &interfaces,
        &floating_ips,
    )))
}

/// Adds `ipv4Address`, `ipv6Address`, `floatingIpUrn` and `floatingIpAddress`
/// to every entry of every instance's `networkInterfaceAttachments`.
///
/// `output` is `{"compute": {"instance": {...}}}` or
/// `{"compute": {"instances": [...]}}`; `interfaces` and `floating_ips` are
/// the network service's list responses. An attachment whose interface is
/// not (or no longer) listed gets `null`s rather than being dropped, so the
/// attachment itself stays visible.
pub(crate) fn join(mut output: Value, interfaces: &Value, floating_ips: &Value) -> Value {
    let by_urn: HashMap<&str, &Value> = list(interfaces, "networkInterfaces")
        .filter_map(|nic| Some((nic.get("urn")?.as_str()?, nic)))
        .collect();
    let fip_by_nic: HashMap<&str, &Value> = list(floating_ips, "floatingIps")
        .filter_map(|fip| Some((fip.get("networkInterfaceUrn")?.as_str()?, fip)))
        .collect();

    let field = |value: Option<&&Value>, key: &str| -> Value {
        value
            .and_then(|v| v.get(key))
            .cloned()
            .unwrap_or(Value::Null)
    };

    let mut fill = |instance: &mut Value| {
        let Some(attachments) = instance
            .get_mut("networkInterfaceAttachments")
            .and_then(Value::as_array_mut)
        else {
            return;
        };
        for attachment in attachments {
            let Some(obj) = attachment.as_object_mut() else {
                continue;
            };
            let nic_urn = obj
                .get("networkInterfaceUrn")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let nic = nic_urn.as_deref().and_then(|urn| by_urn.get(urn));
            let fip = nic_urn.as_deref().and_then(|urn| fip_by_nic.get(urn));
            insert(obj, "ipv4Address", field(nic, "ipv4Address"));
            insert(obj, "ipv6Address", field(nic, "ipv6Address"));
            insert(obj, "floatingIpUrn", field(fip, "urn"));
            insert(obj, "floatingIpAddress", field(fip, "address"));
        }
    };

    if let Some(compute) = output.get_mut("compute") {
        if let Some(instance) = compute.get_mut("instance") {
            if !instance.is_null() {
                fill(instance);
            }
        }
        if let Some(instances) = compute.get_mut("instances").and_then(Value::as_array_mut) {
            instances.iter_mut().for_each(&mut fill);
        }
    }
    output
}

fn list<'a>(response: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    response
        .get("network")
        .and_then(|network| network.get(key))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn insert(obj: &mut Map<String, Value>, key: &str, value: Value) {
    obj.insert(key.to_string(), value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn interfaces() -> Value {
        json!({"network": {"networkInterfaces": [
            {"urn": "nic-a", "ipv4Address": "192.0.2.5", "ipv6Address": "fd00::5"},
            {"urn": "nic-b", "ipv4Address": "192.0.2.6", "ipv6Address": "fd00::6"},
        ]}})
    }

    fn floating_ips() -> Value {
        json!({"network": {"floatingIps": [
            {"urn": "fip-1", "address": "203.0.113.10", "networkInterfaceUrn": "nic-a"},
            {"urn": "fip-2", "address": "203.0.113.11", "networkInterfaceUrn": null},
        ]}})
    }

    #[test]
    fn get_instance_gains_private_and_floating_addresses() {
        let out = join(
            json!({"compute": {"instance": {"urn": "i-1", "networkInterfaceAttachments": [
                {"deviceIndex": 0, "networkInterfaceUrn": "nic-a"}
            ]}}}),
            &interfaces(),
            &floating_ips(),
        );
        let att = &out["compute"]["instance"]["networkInterfaceAttachments"][0];
        assert_eq!(att["ipv4Address"], "192.0.2.5");
        assert_eq!(att["ipv6Address"], "fd00::5");
        assert_eq!(att["floatingIpUrn"], "fip-1");
        assert_eq!(att["floatingIpAddress"], "203.0.113.10");
        assert_eq!(att["deviceIndex"], 0, "existing fields are kept");
    }

    #[test]
    fn list_instances_fills_every_instance_and_leaves_no_floating_ip_as_null() {
        let out = join(
            json!({"compute": {"instances": [
                {"networkInterfaceAttachments": [{"networkInterfaceUrn": "nic-a"}]},
                {"networkInterfaceAttachments": [{"networkInterfaceUrn": "nic-b"}]},
            ]}}),
            &interfaces(),
            &floating_ips(),
        );
        assert_eq!(
            out["compute"]["instances"][0]["networkInterfaceAttachments"][0]["floatingIpAddress"],
            "203.0.113.10"
        );
        let second = &out["compute"]["instances"][1]["networkInterfaceAttachments"][0];
        assert_eq!(second["ipv4Address"], "192.0.2.6");
        assert_eq!(second["floatingIpAddress"], Value::Null);
    }

    #[test]
    fn an_attachment_without_a_listed_interface_keeps_its_place_with_nulls() {
        let out = join(
            json!({"compute": {"instance": {"networkInterfaceAttachments": [
                {"networkInterfaceUrn": null, "status": "INTERFACE_REQUESTED"},
                {"networkInterfaceUrn": "nic-gone"},
            ]}}}),
            &interfaces(),
            &floating_ips(),
        );
        let atts = out["compute"]["instance"]["networkInterfaceAttachments"]
            .as_array()
            .unwrap();
        assert_eq!(atts.len(), 2);
        for att in atts {
            assert_eq!(att["ipv4Address"], Value::Null);
            assert_eq!(att["floatingIpAddress"], Value::Null);
        }
        assert_eq!(atts[0]["status"], "INTERFACE_REQUESTED");
    }

    #[test]
    fn a_missing_instance_passes_through() {
        let out = join(
            json!({"compute": {"instance": null}}),
            &interfaces(),
            &floating_ips(),
        );
        assert_eq!(out, json!({"compute": {"instance": null}}));
    }

    #[test]
    fn the_project_is_derived_from_the_instance_urn() {
        let instance: Urn = str::parse(
            "urn:origin:cloud:compute:eu-central1:0a1b2c3d4e5f:my-project-123:instance/abc123",
        )
        .unwrap();
        // The canonical project URN carries no region: the form
        // `create-project` prints.
        assert_eq!(
            project_of(&instance).unwrap().to_string(),
            "urn:origin:cloud:admin::0a1b2c3d4e5f::my-project-123"
        );
    }

    #[test]
    fn a_urn_without_a_project_is_refused() {
        let org_scoped: Result<Urn, _> =
            "urn:origin:cloud:iam::0a1b2c3d4e5f::tc/role/roles/owner".parse();
        if let Ok(urn) = org_scoped {
            assert!(project_of(&urn).is_err());
        }
    }
}
