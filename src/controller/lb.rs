//! Reconciler logic for Harvester LoadBalancer resources.

use std::sync::Arc;
use std::time::Duration;

use kube::runtime::controller::Action;
use kube::ResourceExt;
use tracing::{debug, error, info, warn};

use super::Context;
use crate::error::{ReconcileError, Result};
use crate::kubernetes::{lb_address, lb_cluster_name, HarvesterLB};
use crate::registry::{ClaimSource, HostnameClaim};

/// Called by the LB controller whenever a LoadBalancer is created, updated, or deleted.
pub async fn reconcile(lb: Arc<HarvesterLB>, ctx: Arc<Context>) -> Result<Action> {
    let name = lb.name_any();
    let namespace = lb.namespace().unwrap_or_default();

    info!(name = %name, namespace = %namespace, "Reconciling LoadBalancer");

    // Get the cluster name from labels
    let cluster_name = match lb_cluster_name(&lb) {
        Some(name) => name.to_lowercase(),
        None => {
            debug!(name = %name, "LoadBalancer has no cluster label, skipping");
            return Ok(Action::await_change());
        }
    };

    let claim_source = ClaimSource::LoadBalancer {
        name: name.clone(),
        namespace: namespace.clone(),
    };

    // -----------------------------------------------------------------------
    // Deletion path
    // -----------------------------------------------------------------------
    if lb.metadata.deletion_timestamp.is_some() {
        info!(
            lb = %name,
            cluster = %cluster_name,
            "LoadBalancer is being deleted — removing DNS claim"
        );
        ctx.registry
            .remove(&cluster_name, &claim_source)
            .await
            .map_err(ReconcileError::RouterOs)?;

        return Ok(Action::await_change());
    }

    // -----------------------------------------------------------------------
    // Normal path — check if address is allocated
    // -----------------------------------------------------------------------
    let address = match lb_address(&lb) {
        Some(addr) => addr.to_string(),
        None => {
            // No address yet — check back later
            warn!(
                lb = %name,
                cluster = %cluster_name,
                "LoadBalancer has no address yet — requeuing in 30s"
            );
            return Ok(Action::requeue(Duration::from_secs(30)));
        }
    };

    info!(
        lb = %name,
        cluster = %cluster_name,
        address = %address,
        "Upserting DNS claim for LoadBalancer"
    );

    ctx.registry
        .upsert(
            &cluster_name,
            HostnameClaim {
                ip: address,
                source: claim_source,
            },
        )
        .await
        .map_err(ReconcileError::RouterOs)?;

    // Steady-state requeue for periodic refresh
    Ok(Action::requeue(Duration::from_secs(600)))
}

/// Called when reconcile returns an error.
pub fn error_policy(lb: Arc<HarvesterLB>, err: &ReconcileError, _ctx: Arc<Context>) -> Action {
    error!(
        name = %lb.name_any(),
        error = %err,
        "LoadBalancer reconcile failed"
    );
    Action::requeue(Duration::from_secs(30))
}
