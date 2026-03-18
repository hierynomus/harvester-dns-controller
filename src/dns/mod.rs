//! DNS client abstraction.
//!
//! This module defines the `DnsClient` trait which abstracts DNS record management.
//! Different implementations can be provided for various DNS backends (RouterOS, etc.).

use anyhow::Result;
use async_trait::async_trait;

#[cfg(test)]
pub mod mock;

/// Trait for DNS record management.
///
/// Implementations of this trait handle creating, updating, and deleting DNS A records.
/// The trait is object-safe and can be used with `Arc<dyn DnsClient>` for runtime
/// polymorphism.
#[async_trait]
pub trait DnsClient: Send + Sync {
    /// Ensure an A record exists for the given FQDN pointing at the given IP.
    ///
    /// If a record with that name already exists (and is managed by this client),
    /// update it to point to the new IP. If no record exists, create one.
    async fn ensure_record(&self, fqdn: &str, ip: &str, ttl: &str) -> Result<()>;

    /// Delete the DNS record for the given FQDN if it exists and is managed by this client.
    async fn delete_record_for_fqdn(&self, fqdn: &str) -> Result<()>;

    /// Remove all DNS records whose FQDNs are NOT in the provided set.
    ///
    /// Used during garbage collection to clean up stale records for VMs that
    /// no longer exist.
    async fn garbage_collect(&self, active_fqdns: &[String]) -> Result<()>;
}
