//! Reconciler logic for VirtualMachineNetworkConfig resources.

use std::sync::Arc;
use std::time::Duration;

use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use tracing::{debug, error, info, warn};

use super::finalizer::{add_finalizer, has_finalizer, remove_finalizer};
use super::Context;
use crate::error::{ReconcileError, Result};
use crate::kubernetes::VmNetworkConfig;

/// Called by the controller whenever a VirtualMachineNetworkConfig is created,
/// updated, or deleted. Also called periodically (requeue) for drift detection.
pub async fn reconcile(vmnetcfg: Arc<VmNetworkConfig>, ctx: Arc<Context>) -> Result<Action> {
    let name = vmnetcfg.name_any();
    let namespace = vmnetcfg.namespace().unwrap_or_default();
    let api: Api<VmNetworkConfig> = Api::namespaced(ctx.kube.clone(), &namespace);

    info!(name = %name, namespace = %namespace, "Reconciling VirtualMachineNetworkConfig");

    let vm_name = &vmnetcfg.spec.vm_name;
    let fqdn = ctx.config.fqdn_for(vm_name);

    // -----------------------------------------------------------------------
    // Deletion path — object has a deletionTimestamp, work through our finalizer
    // -----------------------------------------------------------------------
    if vmnetcfg.metadata.deletion_timestamp.is_some() {
        if has_finalizer(&vmnetcfg) {
            info!(vm = %vm_name, fqdn = %fqdn, "VM is being deleted — removing DNS record");
            ctx.dns
                .delete_record_for_fqdn(&fqdn)
                .await
                .map_err(ReconcileError::RouterOs)?;

            remove_finalizer(&api, &name).await?;
            info!(name = %name, "Finalizer removed, deletion can proceed");
        } else {
            debug!(name = %name, "Object deleting but no finalizer — nothing to do");
        }
        return Ok(Action::await_change());
    }

    // -----------------------------------------------------------------------
    // Normal path — ensure finalizer is present
    // -----------------------------------------------------------------------
    if !has_finalizer(&vmnetcfg) {
        add_finalizer(&api, &name, &vmnetcfg).await?;
        // The patch triggers a new reconcile almost immediately; return early.
        return Ok(Action::requeue(Duration::from_secs(1)));
    }

    // -----------------------------------------------------------------------
    // Extract the allocated IP from status.networkConfig
    //
    // We register one A record per VM keyed on the VM name. If a VM has
    // multiple NICs, we use the first one in "Allocated" state. Additional
    // NICs on different networks can be extended here if needed.
    // -----------------------------------------------------------------------
    let allocated_ip = first_allocated_ip(&vmnetcfg);

    match allocated_ip {
        Some(ip) => {
            info!(vm = %vm_name, fqdn = %fqdn, ip = %ip, "Ensuring DNS A record");
            ctx.dns
                .ensure_record(&fqdn, &ip, &ctx.config.dns_ttl)
                .await
                .map_err(ReconcileError::RouterOs)?;

            // Steady-state requeue: acts as a periodic health / drift check.
            Ok(Action::requeue(Duration::from_secs(600)))
        }
        None => {
            // IP not yet allocated — the Harvester DHCP controller may still
            // be processing. Check back in 30 seconds.
            warn!(
                vm = %vm_name,
                namespace = %namespace,
                "No allocated IP in status yet — requeuing in 30s"
            );
            Ok(Action::requeue(Duration::from_secs(30)))
        }
    }
}

/// Called when reconcile returns an error. Returns an action that tells the
/// controller when to retry — kube applies its own exponential backoff on top.
pub fn error_policy(
    vmnetcfg: Arc<VmNetworkConfig>,
    err: &ReconcileError,
    _ctx: Arc<Context>,
) -> Action {
    error!(
        name = %vmnetcfg.name_any(),
        error = %err,
        "Reconcile failed"
    );
    Action::requeue(Duration::from_secs(30))
}

// ---------------------------------------------------------------------------
// Status helpers
// ---------------------------------------------------------------------------

/// Extract the first allocated IP from the VmNetworkConfig status.
pub(crate) fn first_allocated_ip(vmnetcfg: &VmNetworkConfig) -> Option<String> {
    vmnetcfg
        .status
        .as_ref()?
        .network_configs
        .iter()
        .find_map(|nc| {
            // Print the network config status for debugging, since this is often the source of issues.
            debug!(
                mac = %nc.mac_address,
                network = %nc.network_name,
                allocated_ip = ?nc.allocated_ip_address,
                state = ?nc.state,
                "Checking network config status"
            );
            let allocated = nc.state.as_deref().map(|s| s == "Allocated").unwrap_or(false);
            if allocated {
                nc.allocated_ip_address.clone()
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kubernetes::{NetworkConfigEntry, NetworkConfigStatus, VmNetworkConfigStatus};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn make_vmnetcfg(status: Option<VmNetworkConfigStatus>) -> VmNetworkConfig {
        VmNetworkConfig {
            metadata: ObjectMeta {
                name: Some("test-vm".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            spec: crate::kubernetes::VmNetworkConfigSpec_ {
                vm_name: "test-vm".to_string(),
                network_configs: vec![NetworkConfigEntry {
                    mac_address: "00:11:22:33:44:55".to_string(),
                    network_name: "default/vlan1".to_string(),
                }],
            },
            status,
        }
    }

    #[test]
    fn test_first_allocated_ip_none_when_no_status() {
        let vmnetcfg = make_vmnetcfg(None);
        assert_eq!(first_allocated_ip(&vmnetcfg), None);
    }

    #[test]
    fn test_first_allocated_ip_none_when_empty_network_config() {
        let vmnetcfg = make_vmnetcfg(Some(VmNetworkConfigStatus {
            network_configs: vec![],
        }));
        assert_eq!(first_allocated_ip(&vmnetcfg), None);
    }

    #[test]
    fn test_first_allocated_ip_none_when_pending() {
        let vmnetcfg = make_vmnetcfg(Some(VmNetworkConfigStatus {
            network_configs: vec![NetworkConfigStatus {
                mac_address: "00:11:22:33:44:55".to_string(),
                network_name: "default/vlan1".to_string(),
                allocated_ip_address: None,
                state: Some("Pending".to_string()),
            }],
        }));
        assert_eq!(first_allocated_ip(&vmnetcfg), None);
    }

    #[test]
    fn test_first_allocated_ip_returns_allocated() {
        let vmnetcfg = make_vmnetcfg(Some(VmNetworkConfigStatus {
            network_configs: vec![NetworkConfigStatus {
                mac_address: "00:11:22:33:44:55".to_string(),
                network_name: "default/vlan1".to_string(),
                allocated_ip_address: Some("192.168.1.100".to_string()),
                state: Some("Allocated".to_string()),
            }],
        }));
        assert_eq!(
            first_allocated_ip(&vmnetcfg),
            Some("192.168.1.100".to_string())
        );
    }

    #[test]
    fn test_first_allocated_ip_skips_non_allocated() {
        let vmnetcfg = make_vmnetcfg(Some(VmNetworkConfigStatus {
            network_configs: vec![
                NetworkConfigStatus {
                    mac_address: "00:11:22:33:44:55".to_string(),
                    network_name: "default/vlan1".to_string(),
                    allocated_ip_address: Some("10.0.0.1".to_string()),
                    state: Some("Pending".to_string()),
                },
                NetworkConfigStatus {
                    mac_address: "00:11:22:33:44:66".to_string(),
                    network_name: "default/vlan2".to_string(),
                    allocated_ip_address: Some("192.168.1.100".to_string()),
                    state: Some("Allocated".to_string()),
                },
            ],
        }));
        assert_eq!(
            first_allocated_ip(&vmnetcfg),
            Some("192.168.1.100".to_string())
        );
    }
}
