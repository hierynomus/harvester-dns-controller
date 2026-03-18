//! DNS client abstraction.
//!
//! This module defines the `DnsClient` trait which abstracts DNS record management.
//! Different implementations can be provided for various DNS backends (RouterOS, etc.).

use anyhow::Result;
use async_trait::async_trait;

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

#[cfg(test)]
pub mod mock {
    //! Mock DNS client for testing.

    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// A mock DNS client that stores records in memory.
    /// Useful for unit testing without a real DNS backend.
    #[derive(Debug, Default)]
    pub struct MockDnsClient {
        records: Mutex<HashMap<String, String>>,
    }

    impl MockDnsClient {
        pub fn new() -> Self {
            Self::default()
        }

        /// Get a snapshot of all current records.
        pub fn get_records(&self) -> HashMap<String, String> {
            self.records.lock().unwrap().clone()
        }

        /// Check if a record exists.
        pub fn has_record(&self, fqdn: &str) -> bool {
            self.records.lock().unwrap().contains_key(fqdn)
        }

        /// Get the IP for a given FQDN.
        pub fn get_ip(&self, fqdn: &str) -> Option<String> {
            self.records.lock().unwrap().get(fqdn).cloned()
        }
    }

    #[async_trait]
    impl DnsClient for MockDnsClient {
        async fn ensure_record(&self, fqdn: &str, ip: &str, _ttl: &str) -> Result<()> {
            self.records
                .lock()
                .unwrap()
                .insert(fqdn.to_string(), ip.to_string());
            Ok(())
        }

        async fn delete_record_for_fqdn(&self, fqdn: &str) -> Result<()> {
            self.records.lock().unwrap().remove(fqdn);
            Ok(())
        }

        async fn garbage_collect(&self, active_fqdns: &[String]) -> Result<()> {
            let mut records = self.records.lock().unwrap();
            records.retain(|fqdn, _| active_fqdns.contains(fqdn));
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_mock_ensure_record() {
            let client = MockDnsClient::new();
            client
                .ensure_record("vm1.example.com", "192.168.1.100", "15m")
                .await
                .unwrap();

            assert!(client.has_record("vm1.example.com"));
            assert_eq!(client.get_ip("vm1.example.com"), Some("192.168.1.100".to_string()));
        }

        #[tokio::test]
        async fn test_mock_update_record() {
            let client = MockDnsClient::new();
            client
                .ensure_record("vm1.example.com", "192.168.1.100", "15m")
                .await
                .unwrap();
            client
                .ensure_record("vm1.example.com", "192.168.1.200", "15m")
                .await
                .unwrap();

            assert_eq!(client.get_ip("vm1.example.com"), Some("192.168.1.200".to_string()));
        }

        #[tokio::test]
        async fn test_mock_delete_record() {
            let client = MockDnsClient::new();
            client
                .ensure_record("vm1.example.com", "192.168.1.100", "15m")
                .await
                .unwrap();
            client.delete_record_for_fqdn("vm1.example.com").await.unwrap();

            assert!(!client.has_record("vm1.example.com"));
        }

        #[tokio::test]
        async fn test_mock_garbage_collect() {
            let client = MockDnsClient::new();
            client
                .ensure_record("vm1.example.com", "192.168.1.100", "15m")
                .await
                .unwrap();
            client
                .ensure_record("vm2.example.com", "192.168.1.101", "15m")
                .await
                .unwrap();
            client
                .ensure_record("vm3.example.com", "192.168.1.102", "15m")
                .await
                .unwrap();

            // Keep only vm1 and vm3
            client
                .garbage_collect(&["vm1.example.com".to_string(), "vm3.example.com".to_string()])
                .await
                .unwrap();

            assert!(client.has_record("vm1.example.com"));
            assert!(!client.has_record("vm2.example.com"));
            assert!(client.has_record("vm3.example.com"));
        }
    }
}
