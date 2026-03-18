//! Startup garbage collection for stale DNS records.
//!
//! On operator start, list all VMNCs and all RouterOS managed records.
//! Any record whose FQDN doesn't match a live VM gets deleted. This covers
//! the case where the operator was down when a VM was removed.

use kube::{Api, Client};
use tracing::info;

use crate::config::Config;
use crate::dns::DnsClient;
use crate::kubernetes::VmNetworkConfig;

/// Run garbage collection on startup to clean up stale DNS records.
pub async fn garbage_collect_on_startup(
    kube: &Client,
    dns: &dyn DnsClient,
    config: &Config,
) -> anyhow::Result<()> {
    info!("Running startup garbage collection");

    // Collect all live VMNCs across all namespaces
    let api: Api<VmNetworkConfig> = Api::all(kube.clone());
    let vmnetcfgs = api.list(&Default::default()).await?;

    let active_fqdns: Vec<String> = vmnetcfgs
        .items
        .iter()
        .filter(|v| v.metadata.deletion_timestamp.is_none())
        .map(|v| config.fqdn_for(&v.spec.vm_name))
        .collect();

    info!(
        count = active_fqdns.len(),
        "Found live VirtualMachineNetworkConfigs"
    );

    dns.garbage_collect(&active_fqdns).await?;
    info!("Startup garbage collection complete");
    Ok(())
}
