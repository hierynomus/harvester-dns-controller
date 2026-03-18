//! RouterOS DNS Operator
//!
//! A Kubernetes operator that watches Harvester VirtualMachineNetworkConfig resources
//! and automatically creates/updates/deletes DNS records in RouterOS.

pub mod config;
pub mod controller;
pub mod dns;
pub mod error;
pub mod health;
pub mod kubernetes;
pub mod routeros;

// Re-export commonly used types at the crate root
pub use config::Config;
pub use controller::{garbage_collect_on_startup, reconcile, error_policy, Context};
pub use dns::DnsClient;
pub use error::{ReconcileError, Result};
pub use kubernetes::VmNetworkConfig;
pub use routeros::RouterOsClient;
