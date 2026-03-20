//! DNS Controller for Kubernetes
//!
//! A Kubernetes operator that watches Harvester VirtualMachineNetworkConfig and
//! LoadBalancer resources and automatically creates/updates/deletes DNS records
//! in RouterOS or GL.Inet routers.

pub mod config;
pub mod controller;
pub mod dns;
pub mod error;
pub mod health;
pub mod kubernetes;
pub mod registry;

// Re-export commonly used types at the crate root
pub use config::{Config, DnsBackend};
pub use controller::{garbage_collect_on_startup, run_controllers, Context};
pub use dns::{DnsClient, GlInetClient, RouterOsClient};
pub use error::{ReconcileError, Result};
pub use kubernetes::{HarvesterLB, VmNetworkConfig};
pub use registry::{ClaimSource, HostnameClaim, HostnameRegistry};
