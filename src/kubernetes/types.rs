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

/// Spec of VirtualMachineNetworkConfig.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VmNetworkConfigSpec {
    pub vm_name: String,
    pub network_config: Vec<NetworkConfigEntry>,
}

/// Per-NIC status entry, populated by the dhcp-controller once an IP is allocated.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfigStatus {
    pub mac_address: String,
    pub network_name: String,
    pub allocated_ip_address: Option<String>,
    pub state: Option<String>,
}

/// Status of VirtualMachineNetworkConfig.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct VmNetworkConfigStatus {
    #[serde(default)]
    pub network_config: Vec<NetworkConfigStatus>,
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
    pub network_config: Vec<NetworkConfigEntry>,
}

// Type alias so the rest of the code can use a clean name.
pub type VmNetworkConfig = VirtualMachineNetworkConfig;

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
            "allocatedIpAddress": "192.168.1.100",
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
        assert!(status.network_config.is_empty());
    }

    #[test]
    fn test_vmnetcfg_spec_deserialize() {
        let json = r#"{
            "vmName": "my-test-vm",
            "networkConfig": [
                {"macAddress": "00:11:22:33:44:55", "networkName": "default/vlan1"}
            ]
        }"#;
        let spec: VmNetworkConfigSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.vm_name, "my-test-vm");
        assert_eq!(spec.network_config.len(), 1);
        assert_eq!(spec.network_config[0].mac_address, "00:11:22:33:44:55");
    }
}
