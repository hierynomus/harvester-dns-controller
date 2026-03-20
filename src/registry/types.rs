//! Types for hostname claim management.

/// Action taken on a DNS record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsAction {
    /// A new DNS record was created.
    Created,
    /// An existing DNS record was updated with a new IP.
    Updated,
    /// The DNS record was already correct (no change).
    Unchanged,
    /// The DNS record was deleted.
    Deleted,
    /// No action taken (e.g., claim was not winning).
    None,
}

impl DnsAction {
    /// Whether any DNS operation occurred.
    pub fn is_change(&self) -> bool {
        matches!(self, DnsAction::Created | DnsAction::Updated | DnsAction::Deleted)
    }
}

/// Source of a hostname claim.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClaimSource {
    /// Claim from a VirtualMachineNetworkConfig.
    VirtualMachine { name: String, namespace: String },
    /// Claim from a Harvester LoadBalancer.
    LoadBalancer { name: String, namespace: String },
}

impl ClaimSource {
    /// Get the priority of this source. Higher wins.
    pub fn priority(&self) -> u8 {
        match self {
            ClaimSource::VirtualMachine { .. } => 5,
            ClaimSource::LoadBalancer { .. } => 10,
        }
    }

    /// Get a human-readable type name.
    pub fn kind(&self) -> &'static str {
        match self {
            ClaimSource::VirtualMachine { .. } => "VirtualMachine",
            ClaimSource::LoadBalancer { .. } => "LoadBalancer",
        }
    }
}

/// A claim on a hostname.
#[derive(Debug, Clone)]
pub struct HostnameClaim {
    pub ip: String,
    pub source: ClaimSource,
}
