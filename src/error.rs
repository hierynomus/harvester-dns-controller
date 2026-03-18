//! Error types for the routeros-dns-operator.

/// Errors that can occur during reconciliation of VirtualMachineNetworkConfig resources.
#[derive(thiserror::Error, Debug)]
pub enum ReconcileError {
    #[error("RouterOS API error: {0}")]
    RouterOs(#[from] anyhow::Error),

    #[error("Kubernetes API error: {0}")]
    Kube(#[from] kube::Error),

    #[error("Serialisation error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T, E = ReconcileError> = std::result::Result<T, E>;
