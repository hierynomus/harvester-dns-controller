//! Kubernetes CRD types for Harvester VirtualMachineNetworkConfig.

mod types;

pub use types::{
    NetworkConfigEntry, NetworkConfigStatus, VirtualMachineNetworkConfig, VmNetworkConfig,
    VmNetworkConfigSpec, VmNetworkConfigSpec_, VmNetworkConfigStatus,
};
