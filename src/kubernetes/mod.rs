//! Kubernetes CRD types for Harvester VirtualMachineNetworkConfig and LoadBalancer.

mod types;

pub use types::{
    derive_hostname, lb_address, lb_cluster_name, HarvesterLB, HarvesterLoadBalancerStatus,
    NetworkConfigEntry, NetworkConfigStatus, VirtualMachine, VirtualMachineNetworkConfig,
    VmNetworkConfig, VmNetworkConfigSpec_, VmNetworkConfigStatus, GUEST_CLUSTER_LABEL,
    HOSTNAME_ANNOTATION, LB_CLUSTER_LABEL,
};
