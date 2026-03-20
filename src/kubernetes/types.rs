//! Harvester VirtualMachineNetworkConfig CRD types.
//!
//! We define just enough of the spec/status to work with — kube will ignore
//! unknown fields from the actual cluster.

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Mirrors the networkConfig entry inside the spec.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfigEntry {
    pub mac_address: String,
    pub network_name: String,
}

/// Per-NIC status entry, populated by the dhcp-controller once an IP is allocated.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfigStatus {
    pub mac_address: String,
    pub network_name: String,
    #[serde(rename = "allocatedIPAddress")]
    pub allocated_ip_address: Option<String>,
    pub state: Option<String>,
}

/// Status of VirtualMachineNetworkConfig.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct VmNetworkConfigStatus {
    #[serde(default)]
    pub network_configs: Vec<NetworkConfigStatus>,
}

/// The full VirtualMachineNetworkConfig custom resource.
/// `group` and `version` match Harvester's actual API group.
#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "network.harvesterhci.io",
    version = "v1alpha1",
    kind = "VirtualMachineNetworkConfig",
    shortname = "vmnetcfg",
    namespaced,
    status = "VmNetworkConfigStatus"
)]
#[serde(rename_all = "camelCase")]
pub struct VmNetworkConfigSpec_ {
    pub vm_name: String,
    pub network_configs: Vec<NetworkConfigEntry>,
}

// Type alias so the rest of the code can use a clean name.
pub type VmNetworkConfig = VirtualMachineNetworkConfig;

/// Label key for Harvester guest cluster name on VirtualMachine resources.
pub const GUEST_CLUSTER_LABEL: &str = "guestcluster.harvesterhci.io/name";

/// Annotation key for overriding the DNS hostname.
pub const HOSTNAME_ANNOTATION: &str = "dns.geeko.me/hostname";

/// Minimal VirtualMachine type for querying labels.
/// We only need metadata, so spec uses a catch-all for unknown fields.
#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "kubevirt.io",
    version = "v1",
    kind = "VirtualMachine",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMachineSpec {
    /// Catch-all for spec fields we don't need to parse.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Harvester LoadBalancer CRD
// ---------------------------------------------------------------------------

/// Label key for the cluster name on Harvester LoadBalancer resources.
pub const LB_CLUSTER_LABEL: &str = "cloudprovider.harvesterhci.io/cluster";

/// Harvester LoadBalancer spec — we only need minimal fields.
#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "loadbalancer.harvesterhci.io",
    version = "v1beta1",
    kind = "LoadBalancer",
    namespaced,
    status = "HarvesterLoadBalancerStatus"
)]
#[serde(rename_all = "camelCase")]
pub struct HarvesterLoadBalancerSpec {
    /// Catch-all for spec fields we don't need to parse.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Status of a Harvester LoadBalancer.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct HarvesterLoadBalancerStatus {
    /// The allocated external IP address.
    #[serde(default)]
    pub address: Option<String>,
}

/// Type alias - the macro generates `LoadBalancer`, we alias it for clarity.
pub type HarvesterLB = LoadBalancer;

/// Extract the cluster name from a LoadBalancer's labels.
pub fn lb_cluster_name(lb: &HarvesterLB) -> Option<&str> {
    lb.metadata
        .labels
        .as_ref()?
        .get(LB_CLUSTER_LABEL)
        .map(|s: &String| s.as_str())
}

/// Extract the allocated IP from a LoadBalancer's status.
pub fn lb_address(lb: &HarvesterLB) -> Option<&str> {
    lb.status.as_ref()?.address.as_deref()
}

// ---------------------------------------------------------------------------
// Hostname derivation
// ---------------------------------------------------------------------------

/// Derive the DNS hostname for a VM.
///
/// Priority:
/// 1. Annotation `dns.geeko.me/hostname` on VMNC → use that
/// 2. Label `guestcluster.harvesterhci.io/name` on VM → use cluster name (if enabled)
/// 3. Fall back to VM name
pub fn derive_hostname(
    vmnetcfg: &VmNetworkConfig,
    vm_labels: Option<&std::collections::BTreeMap<String, String>>,
    use_guest_cluster_label: bool,
) -> String {
    // Priority 1: Explicit annotation override
    if let Some(annotations) = &vmnetcfg.metadata.annotations {
        if let Some(hostname) = annotations.get(HOSTNAME_ANNOTATION) {
            return hostname.to_lowercase();
        }
    }

    // Priority 2: Guest cluster label on VM → cluster name as hostname
    if use_guest_cluster_label {
        if let Some(labels) = vm_labels {
            if let Some(cluster_name) = labels.get(GUEST_CLUSTER_LABEL) {
                return cluster_name.to_lowercase();
            }
        }
    }

    // Priority 3: Fall back to VM name
    vmnetcfg.spec.vm_name.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_entry_deserialize() {
        let json = r#"{"macAddress": "00:11:22:33:44:55", "networkName": "default/vlan1"}"#;
        let entry: NetworkConfigEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.mac_address, "00:11:22:33:44:55");
        assert_eq!(entry.network_name, "default/vlan1");
    }

    #[test]
    fn test_network_config_status_deserialize() {
        let json = r#"{
            "macAddress": "00:11:22:33:44:55",
            "networkName": "default/vlan1",
            "allocatedIPAddress": "192.168.1.100",
            "state": "Allocated"
        }"#;
        let status: NetworkConfigStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.mac_address, "00:11:22:33:44:55");
        assert_eq!(status.allocated_ip_address, Some("192.168.1.100".to_string()));
        assert_eq!(status.state, Some("Allocated".to_string()));
    }

    #[test]
    fn test_network_config_status_deserialize_minimal() {
        let json = r#"{"macAddress": "00:11:22:33:44:55", "networkName": "default/vlan1"}"#;
        let status: NetworkConfigStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.mac_address, "00:11:22:33:44:55");
        assert_eq!(status.allocated_ip_address, None);
        assert_eq!(status.state, None);
    }

    #[test]
    fn test_vmnetcfg_status_default() {
        let status = VmNetworkConfigStatus::default();
        assert!(status.network_configs.is_empty());
    }

    #[test]
    fn test_vmnetcfg_spec_deserialize() {
        let json = r#"{
            "vmName": "my-test-vm",
            "networkConfigs": [
                {"macAddress": "00:11:22:33:44:55", "networkName": "default/vlan1"}
            ]
        }"#;
        let spec: VmNetworkConfigSpec_ = serde_json::from_str(json).unwrap();
        assert_eq!(spec.vm_name, "my-test-vm");
        assert_eq!(spec.network_configs.len(), 1);
        assert_eq!(spec.network_configs[0].mac_address, "00:11:22:33:44:55");
    }

    fn make_vmnetcfg_for_hostname_test(
        vm_name: &str,
        annotations: Option<std::collections::BTreeMap<String, String>>,
    ) -> VmNetworkConfig {
        VmNetworkConfig {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(vm_name.to_string()),
                namespace: Some("default".to_string()),
                annotations,
                ..Default::default()
            },
            spec: VmNetworkConfigSpec_ {
                vm_name: vm_name.to_string(),
                network_configs: vec![],
            },
            status: None,
        }
    }

    #[test]
    fn test_derive_hostname_fallback_to_vm_name() {
        let vmnetcfg = make_vmnetcfg_for_hostname_test("my-test-vm", None);
        let hostname = derive_hostname(&vmnetcfg, None, true);
        assert_eq!(hostname, "my-test-vm");
    }

    #[test]
    fn test_derive_hostname_from_guest_cluster_label() {
        let vmnetcfg = make_vmnetcfg_for_hostname_test("pollux-pool1-abc123", None);
        let mut labels = std::collections::BTreeMap::new();
        labels.insert(GUEST_CLUSTER_LABEL.to_string(), "pollux".to_string());

        let hostname = derive_hostname(&vmnetcfg, Some(&labels), true);
        assert_eq!(hostname, "pollux");
    }

    #[test]
    fn test_derive_hostname_guest_cluster_label_disabled() {
        let vmnetcfg = make_vmnetcfg_for_hostname_test("pollux-pool1-abc123", None);
        let mut labels = std::collections::BTreeMap::new();
        labels.insert(GUEST_CLUSTER_LABEL.to_string(), "pollux".to_string());

        // With use_guest_cluster_label = false, should fall back to VM name
        let hostname = derive_hostname(&vmnetcfg, Some(&labels), false);
        assert_eq!(hostname, "pollux-pool1-abc123");
    }

    #[test]
    fn test_derive_hostname_annotation_override() {
        let mut annotations = std::collections::BTreeMap::new();
        annotations.insert(HOSTNAME_ANNOTATION.to_string(), "custom-host".to_string());
        let vmnetcfg = make_vmnetcfg_for_hostname_test("pollux-pool1-abc123", Some(annotations));

        let mut labels = std::collections::BTreeMap::new();
        labels.insert(GUEST_CLUSTER_LABEL.to_string(), "pollux".to_string());

        // Annotation should take priority over guest cluster label
        let hostname = derive_hostname(&vmnetcfg, Some(&labels), true);
        assert_eq!(hostname, "custom-host");
    }

    #[test]
    fn test_derive_hostname_lowercase() {
        let vmnetcfg = make_vmnetcfg_for_hostname_test("MyTestVM", None);
        let hostname = derive_hostname(&vmnetcfg, None, true);
        assert_eq!(hostname, "mytestvm");
    }
}
