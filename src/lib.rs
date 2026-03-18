//! RouterOS DNS Operator
//!
//! A Kubernetes operator that watches Harvester VirtualMachineNetworkConfig and
//! LoadBalancer resources and automatically creates/updates/deletes DNS records in RouterOS.

pub mod config;
pub mod controller;
pub mod dns;
pub mod error;
pub mod health;
pub mod kubernetes;
pub mod registry;
pub mod routeros;

// Re-export commonly used types at the crate root
pub use config::Config;
pub use controller::{garbage_collect_on_startup, run_controllers, Context};
pub use dns::DnsClient;
pub use error::{ReconcileError, Result};
pub use kubernetes::{HarvesterLB, VmNetworkConfig};
pub use registry::{ClaimSource, HostnameClaim, HostnameRegistry};
pub use routeros::RouterOsClient;
