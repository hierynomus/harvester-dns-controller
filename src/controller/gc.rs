//! Startup garbage collection for stale DNS records.
//!
//! On operator start, list all VMNCs and LBs, populate the registry,
//! then garbage collect any DNS records not in the registry.

use kube::{Api, Client, ResourceExt};
use tracing::info;

use super::helpers::get_vm_labels;
use crate::config::Config;
use crate::kubernetes::{derive_hostname, lb_address, lb_cluster_name, HarvesterLB, VmNetworkConfig};
use crate::registry::{ClaimSource, HostnameClaim, HostnameRegistry};

/// Run garbage collection on startup to sync registry and clean up stale DNS records.
pub async fn garbage_collect_on_startup(
    kube: &Client,
    registry: &HostnameRegistry,
    config: &Config,
) -> anyhow::Result<()> {
    info!("Running startup garbage collection");

    // -----------------------------------------------------------------------
    // Phase 1: Populate registry from existing VMNCs
    // -----------------------------------------------------------------------
    let vmnc_api: Api<VmNetworkConfig> = Api::all(kube.clone());
    let vmnetcfgs = vmnc_api.list(&Default::default()).await?;

    let mut vmnc_count = 0;
    for vmnetcfg in vmnetcfgs.items.iter() {
        if vmnetcfg.metadata.deletion_timestamp.is_some() {
            continue;
        }

        let name = vmnetcfg.name_any();
        let namespace = vmnetcfg.namespace().unwrap_or_default();

        // Look up VM labels for hostname derivation
        let vm_labels = get_vm_labels(kube, vmnetcfg).await;

        let hostname = derive_hostname(vmnetcfg, vm_labels.as_ref(), config.dns_use_guest_cluster_label);

        // Get allocated IP
        let ip = vmnetcfg
            .status
            .as_ref()
            .and_then(|s| {
                s.network_configs.iter().find_map(|nc| {
                    if nc.state.as_deref() == Some("Allocated") {
                        nc.allocated_ip_address.clone()
                    } else {
                        None
                    }
                })
            });

        if let Some(ip) = ip {
            registry
                .upsert(
                    &hostname,
                    HostnameClaim {
                        ip,
                        source: ClaimSource::VirtualMachine {
                            name: name.clone(),
                            namespace: namespace.clone(),
                        },
                    },
                )
                .await?;
            vmnc_count += 1;
        }
    }

    info!(count = vmnc_count, "Populated registry from VMNCs");

    // -----------------------------------------------------------------------
    // Phase 2: Populate registry from existing LoadBalancers
    // -----------------------------------------------------------------------
    let lb_api: Api<HarvesterLB> = Api::all(kube.clone());
    let lbs = lb_api.list(&Default::default()).await?;

    let mut lb_count = 0;
    for lb in lbs.items.iter() {
        if lb.metadata.deletion_timestamp.is_some() {
            continue;
        }

        let name = lb.name_any();
        let namespace = lb.namespace().unwrap_or_default();

        let cluster_name = match lb_cluster_name(lb) {
            Some(c) => c.to_lowercase(),
            None => continue,
        };

        let address = match lb_address(lb) {
            Some(a) => a.to_string(),
            None => continue,
        };

        registry
            .upsert(
                &cluster_name,
                HostnameClaim {
                    ip: address,
                    source: ClaimSource::LoadBalancer {
                        name: name.clone(),
                        namespace: namespace.clone(),
                    },
                },
            )
            .await?;
        lb_count += 1;
    }

    info!(count = lb_count, "Populated registry from LoadBalancers");

    // -----------------------------------------------------------------------
    // Phase 3: Garbage collect DNS records not in registry
    // -----------------------------------------------------------------------
    registry.garbage_collect().await?;

    info!("Startup garbage collection complete");
    Ok(())
}
