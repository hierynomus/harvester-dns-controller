//! Finalizer helpers for VirtualMachineNetworkConfig resources.

use kube::api::{Patch, PatchParams};
use kube::Api;
use serde_json::json;
use tracing::debug;

use crate::error::Result;
use crate::kubernetes::VmNetworkConfig;

/// The finalizer we attach to every VMNC we manage. This ensures Kubernetes
/// holds deletion open until we've had a chance to remove the DNS record.
pub const FINALIZER: &str = "dns.routeros.geeko.me/cleanup";

/// Check if the resource has our finalizer attached.
pub fn has_finalizer(vmnetcfg: &VmNetworkConfig) -> bool {
    vmnetcfg
        .metadata
        .finalizers
        .as_ref()
        .map(|f| f.iter().any(|s| s == FINALIZER))
        .unwrap_or(false)
}

/// Add our finalizer to the resource.
pub async fn add_finalizer(
    api: &Api<VmNetworkConfig>,
    name: &str,
    vmnetcfg: &VmNetworkConfig,
) -> Result<()> {
    let mut finalizers = vmnetcfg.metadata.finalizers.clone().unwrap_or_default();
    finalizers.push(FINALIZER.to_string());

    let patch = json!({
        "metadata": {
            "finalizers": finalizers
        }
    });

    api.patch(
        name,
        &PatchParams::apply("harvester-dns-controller"),
        &Patch::Merge(&patch),
    )
    .await?;

    debug!(name = %name, finalizer = FINALIZER, "Finalizer added");
    Ok(())
}

/// Remove our finalizer from the resource.
pub async fn remove_finalizer(api: &Api<VmNetworkConfig>, name: &str) -> Result<()> {
    // Strategic merge patch to remove our specific finalizer
    // Use JSON Merge Patch: set finalizers to the list minus ours.
    // We fetch the current object first to get an accurate list.
    let current = api.get(name).await?;
    let remaining: Vec<String> = current
        .metadata
        .finalizers
        .unwrap_or_default()
        .into_iter()
        .filter(|f| f != FINALIZER)
        .collect();

    let patch = json!({
        "metadata": {
            "finalizers": remaining
        }
    });

    api.patch(
        name,
        &PatchParams::apply("harvester-dns-controller"),
        &Patch::Merge(&patch),
    )
    .await?;

    debug!(name = %name, finalizer = FINALIZER, "Finalizer removed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kubernetes::{NetworkConfigEntry, VmNetworkConfigSpec_};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn make_vmnetcfg(finalizers: Option<Vec<String>>) -> VmNetworkConfig {
        VmNetworkConfig {
            metadata: ObjectMeta {
                name: Some("test-vm".to_string()),
                namespace: Some("default".to_string()),
                finalizers,
                ..Default::default()
            },
            spec: VmNetworkConfigSpec_ {
                vm_name: "test-vm".to_string(),
                network_configs: vec![NetworkConfigEntry {
                    mac_address: "00:11:22:33:44:55".to_string(),
                    network_name: "default/vlan1".to_string(),
                }],
            },
            status: None,
        }
    }

    #[test]
    fn test_has_finalizer_none() {
        let vmnetcfg = make_vmnetcfg(None);
        assert!(!has_finalizer(&vmnetcfg));
    }

    #[test]
    fn test_has_finalizer_empty() {
        let vmnetcfg = make_vmnetcfg(Some(vec![]));
        assert!(!has_finalizer(&vmnetcfg));
    }

    #[test]
    fn test_has_finalizer_other_finalizer() {
        let vmnetcfg = make_vmnetcfg(Some(vec!["other-finalizer".to_string()]));
        assert!(!has_finalizer(&vmnetcfg));
    }

    #[test]
    fn test_has_finalizer_present() {
        let vmnetcfg = make_vmnetcfg(Some(vec![FINALIZER.to_string()]));
        assert!(has_finalizer(&vmnetcfg));
    }

    #[test]
    fn test_has_finalizer_among_others() {
        let vmnetcfg = make_vmnetcfg(Some(vec![
            "other-finalizer".to_string(),
            FINALIZER.to_string(),
            "another-finalizer".to_string(),
        ]));
        assert!(has_finalizer(&vmnetcfg));
    }
}
