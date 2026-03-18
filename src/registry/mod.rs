//! Hostname registry for managing DNS claims from multiple sources.
//!
//! The registry mediates between different controllers (VMNC, LoadBalancer) that
//! may claim the same hostname. It uses priority to determine which claim wins.

mod manager;
mod types;

pub use manager::HostnameRegistry;
pub use types::{ClaimSource, HostnameClaim};
