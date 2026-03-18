//! Kubernetes controller logic for VirtualMachineNetworkConfig resources.

mod finalizer;
mod gc;
mod reconciler;

use std::sync::Arc;

use kube::Client;

use crate::config::Config;
use crate::dns::DnsClient;

pub use gc::garbage_collect_on_startup;
pub use reconciler::{error_policy, reconcile};

// ---------------------------------------------------------------------------
// Context shared across all reconcile calls
// ---------------------------------------------------------------------------

/// Shared context passed into every reconcile call.
pub struct Context {
    pub config: Config,
    pub dns: Arc<dyn DnsClient>,
    pub kube: Client,
}
