//! Hostname registry implementation.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::dns::DnsClient;

use super::types::{ClaimSource, DnsAction, HostnameClaim};

/// Result of an upsert or remove operation.
#[derive(Debug, Clone)]
pub struct RegistryResult {
    /// The action taken on the DNS record.
    pub action: DnsAction,
    /// The FQDN affected.
    pub fqdn: String,
    /// The IP address (if created/updated).
    pub ip: Option<String>,
}

/// The hostname registry manages claims and updates DNS accordingly.
///
/// Multiple sources can claim the same hostname. The source with the highest
/// priority wins and its IP is used for the DNS record.
pub struct HostnameRegistry {
    /// Map of hostname → list of claims (from different sources).
    claims: RwLock<HashMap<String, Vec<HostnameClaim>>>,
    /// DNS client for creating/updating/deleting records.
    dns: Arc<dyn DnsClient>,
    /// DNS domain suffix (e.g., "lab.example.com").
    domain: String,
    /// TTL for DNS records.
    ttl: String,
}

impl HostnameRegistry {
    /// Create a new hostname registry.
    pub fn new(dns: Arc<dyn DnsClient>, domain: String, ttl: String) -> Self {
        Self {
            claims: RwLock::new(HashMap::new()),
            dns,
            domain,
            ttl,
        }
    }

    /// Build the FQDN for a hostname.
    fn fqdn(&self, hostname: &str) -> String {
        format!("{}.{}", hostname.to_lowercase(), self.domain)
    }

    /// Upsert a claim for a hostname.
    ///
    /// If this source already has a claim, it's updated. Then the DNS record
    /// is updated to reflect the highest-priority claim.
    ///
    /// Returns information about what action was taken for event publishing.
    pub async fn upsert(&self, hostname: &str, claim: HostnameClaim) -> anyhow::Result<RegistryResult> {
        let hostname = hostname.to_lowercase();
        let fqdn = self.fqdn(&hostname);

        let mut claims = self.claims.write().await;
        let entry = claims.entry(hostname.clone()).or_default();

        // Find previous winner before making changes
        let previous_winner = entry.iter().max_by_key(|c| c.source.priority()).cloned();

        // Remove existing claim from the same source (if any)
        entry.retain(|c| c.source != claim.source);

        // Add the new claim
        debug!(
            hostname = %hostname,
            ip = %claim.ip,
            source = ?claim.source,
            "Upserting hostname claim"
        );
        entry.push(claim);

        // Find the winning claim (highest priority)
        let winner = entry.iter().max_by_key(|c| c.source.priority());

        if let Some(winner) = winner {
            // Determine the action based on previous state
            let action = match &previous_winner {
                None => DnsAction::Created,
                Some(prev) if prev.ip != winner.ip => DnsAction::Updated,
                Some(_) => DnsAction::Unchanged,
            };

            if action != DnsAction::Unchanged {
                info!(
                    fqdn = %fqdn,
                    ip = %winner.ip,
                    source_kind = %winner.source.kind(),
                    action = ?action,
                    "Ensuring DNS record for winning claim"
                );
            }

            self.dns.ensure_record(&fqdn, &winner.ip, &self.ttl).await?;

            Ok(RegistryResult {
                action,
                fqdn,
                ip: Some(winner.ip.clone()),
            })
        } else {
            // This shouldn't happen since we just added a claim
            Ok(RegistryResult {
                action: DnsAction::None,
                fqdn,
                ip: None,
            })
        }
    }

    /// Remove a claim from a specific source.
    ///
    /// If other claims remain, the DNS record is updated to the next highest
    /// priority claim. If no claims remain, the DNS record is deleted.
    ///
    /// Returns information about what action was taken for event publishing.
    pub async fn remove(&self, hostname: &str, source: &ClaimSource) -> anyhow::Result<RegistryResult> {
        let hostname = hostname.to_lowercase();
        let fqdn = self.fqdn(&hostname);

        let mut claims = self.claims.write().await;

        if let Some(entry) = claims.get_mut(&hostname) {
            // Find previous winner before removing
            let previous_winner = entry.iter().max_by_key(|c| c.source.priority()).cloned();

            // Remove the claim from this source
            let had_claim = entry.iter().any(|c| &c.source == source);
            entry.retain(|c| &c.source != source);

            if had_claim {
                debug!(
                    hostname = %hostname,
                    source = ?source,
                    remaining = entry.len(),
                    "Removed hostname claim"
                );
            }

            if entry.is_empty() {
                // No more claims — delete the DNS record
                claims.remove(&hostname);
                info!(fqdn = %fqdn, "Deleting DNS record (no claims remain)");
                self.dns.delete_record_for_fqdn(&fqdn).await?;

                return Ok(RegistryResult {
                    action: DnsAction::Deleted,
                    fqdn,
                    ip: previous_winner.map(|w| w.ip),
                });
            } else {
                // Find the new winner
                let winner = entry.iter().max_by_key(|c| c.source.priority());
                if let Some(winner) = winner {
                    // Determine if the winner changed
                    let action = match &previous_winner {
                        Some(prev) if prev.ip != winner.ip => {
                            info!(
                                fqdn = %fqdn,
                                ip = %winner.ip,
                                source_kind = %winner.source.kind(),
                                "Updating DNS record to next winning claim"
                            );
                            DnsAction::Updated
                        }
                        _ => DnsAction::Unchanged,
                    };

                    if action != DnsAction::Unchanged {
                        self.dns.ensure_record(&fqdn, &winner.ip, &self.ttl).await?;
                    }

                    return Ok(RegistryResult {
                        action,
                        fqdn,
                        ip: Some(winner.ip.clone()),
                    });
                }
            }
        }

        Ok(RegistryResult {
            action: DnsAction::None,
            fqdn,
            ip: None,
        })
    }

    /// Get a snapshot of all hostnames with claims (for garbage collection).
    pub async fn active_fqdns(&self) -> Vec<String> {
        let claims = self.claims.read().await;
        claims.keys().map(|h| self.fqdn(h)).collect()
    }

    /// Run garbage collection: remove DNS records not in the registry.
    pub async fn garbage_collect(&self) -> anyhow::Result<()> {
        let active = self.active_fqdns().await;
        info!(count = active.len(), "Running garbage collection");
        self.dns.garbage_collect(&active).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dns::mock::MockDnsClient;

    #[tokio::test]
    async fn test_upsert_single_claim() {
        let dns = Arc::new(MockDnsClient::new());
        let registry =
            HostnameRegistry::new(dns.clone(), "example.com".to_string(), "15m".to_string());

        registry
            .upsert(
                "myvm",
                HostnameClaim {
                    ip: "10.0.0.1".to_string(),
                    source: ClaimSource::VirtualMachine {
                        name: "myvm".to_string(),
                        namespace: "default".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert!(dns.has_record("myvm.example.com"));
        assert_eq!(dns.get_ip("myvm.example.com"), Some("10.0.0.1".to_string()));
    }

    #[tokio::test]
    async fn test_loadbalancer_wins_over_vm() {
        let dns = Arc::new(MockDnsClient::new());
        let registry =
            HostnameRegistry::new(dns.clone(), "example.com".to_string(), "15m".to_string());

        // VM claims first
        registry
            .upsert(
                "cluster",
                HostnameClaim {
                    ip: "10.0.0.1".to_string(),
                    source: ClaimSource::VirtualMachine {
                        name: "cluster-pool1-abc".to_string(),
                        namespace: "default".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(
            dns.get_ip("cluster.example.com"),
            Some("10.0.0.1".to_string())
        );

        // LB claims same hostname — should win
        registry
            .upsert(
                "cluster",
                HostnameClaim {
                    ip: "10.0.0.100".to_string(),
                    source: ClaimSource::LoadBalancer {
                        name: "cluster-traefik".to_string(),
                        namespace: "default".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(
            dns.get_ip("cluster.example.com"),
            Some("10.0.0.100".to_string())
        );
    }

    #[tokio::test]
    async fn test_remove_lb_falls_back_to_vm() {
        let dns = Arc::new(MockDnsClient::new());
        let registry =
            HostnameRegistry::new(dns.clone(), "example.com".to_string(), "15m".to_string());

        let vm_source = ClaimSource::VirtualMachine {
            name: "cluster-pool1-abc".to_string(),
            namespace: "default".to_string(),
        };
        let lb_source = ClaimSource::LoadBalancer {
            name: "cluster-traefik".to_string(),
            namespace: "default".to_string(),
        };

        // VM claims
        registry
            .upsert(
                "cluster",
                HostnameClaim {
                    ip: "10.0.0.1".to_string(),
                    source: vm_source.clone(),
                },
            )
            .await
            .unwrap();

        // LB claims
        registry
            .upsert(
                "cluster",
                HostnameClaim {
                    ip: "10.0.0.100".to_string(),
                    source: lb_source.clone(),
                },
            )
            .await
            .unwrap();

        // LB wins
        assert_eq!(
            dns.get_ip("cluster.example.com"),
            Some("10.0.0.100".to_string())
        );

        // Remove LB claim — should fall back to VM
        registry.remove("cluster", &lb_source).await.unwrap();
        assert_eq!(
            dns.get_ip("cluster.example.com"),
            Some("10.0.0.1".to_string())
        );
    }

    #[tokio::test]
    async fn test_remove_last_claim_deletes_record() {
        let dns = Arc::new(MockDnsClient::new());
        let registry =
            HostnameRegistry::new(dns.clone(), "example.com".to_string(), "15m".to_string());

        let source = ClaimSource::VirtualMachine {
            name: "myvm".to_string(),
            namespace: "default".to_string(),
        };

        registry
            .upsert(
                "myvm",
                HostnameClaim {
                    ip: "10.0.0.1".to_string(),
                    source: source.clone(),
                },
            )
            .await
            .unwrap();

        assert!(dns.has_record("myvm.example.com"));

        registry.remove("myvm", &source).await.unwrap();
        assert!(!dns.has_record("myvm.example.com"));
    }
}
