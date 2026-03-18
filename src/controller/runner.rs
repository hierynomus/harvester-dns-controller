//! Controller runner and shared context.

use std::sync::Arc;

use futures::StreamExt;
use kube::runtime::controller::Error as ControllerError;
use kube::runtime::watcher;
use kube::runtime::Controller;
use kube::{Api, Client};
use tracing::{debug, info};

use crate::config::Config;
use crate::kubernetes::{HarvesterLB, VmNetworkConfig};
use crate::registry::HostnameRegistry;

use super::{lb, vmnc};

/// Shared context passed into every reconcile call.
pub struct Context {
    pub config: Config,
    pub registry: Arc<HostnameRegistry>,
    pub kube: Client,
}

/// Run both VMNC and LoadBalancer controllers concurrently.
pub async fn run_controllers(ctx: Arc<Context>) {
    let kube_client = ctx.kube.clone();

    // Watch VirtualMachineNetworkConfig across all namespaces
    let vmnc_api: Api<VmNetworkConfig> = Api::all(kube_client.clone());

    // Watch LoadBalancer across all namespaces
    let lb_api: Api<HarvesterLB> = Api::all(kube_client.clone());

    let watcher_config = watcher::Config::default().any_semantic();

    let vmnc_ctx = ctx.clone();
    let lb_ctx = ctx.clone();

    // Run both controllers concurrently
    let vmnc_controller = async {
        info!("Starting VMNC controller");
        Controller::new(vmnc_api, watcher_config.clone())
            .run(vmnc::reconcile, vmnc::error_policy, vmnc_ctx)
            .for_each(|result| async move {
                match result {
                    Ok((obj, action)) => {
                        info!(
                            kind = "VMNC",
                            name = %obj.name,
                            namespace = %obj.namespace.as_deref().unwrap_or(""),
                            requeue_after = ?action,
                            "Reconcile OK"
                        );
                    }
                    Err(ControllerError::ObjectNotFound(obj_ref)) => {
                        debug!(
                            kind = "VMNC",
                            name = %obj_ref.name,
                            namespace = ?obj_ref.namespace,
                            "Object already deleted, skipping reconcile"
                        );
                    }
                    Err(err) => {
                        tracing::warn!(kind = "VMNC", error = ?err, "Controller stream error (will retry)");
                    }
                }
            })
            .await;
    };

    let lb_controller = async {
        info!("Starting LoadBalancer controller");
        Controller::new(lb_api, watcher_config.clone())
            .run(lb::reconcile, lb::error_policy, lb_ctx)
            .for_each(|result| async move {
                match result {
                    Ok((obj, action)) => {
                        info!(
                            kind = "LoadBalancer",
                            name = %obj.name,
                            namespace = %obj.namespace.as_deref().unwrap_or(""),
                            requeue_after = ?action,
                            "Reconcile OK"
                        );
                    }
                    Err(ControllerError::ObjectNotFound(obj_ref)) => {
                        debug!(
                            kind = "LoadBalancer",
                            name = %obj_ref.name,
                            namespace = ?obj_ref.namespace,
                            "Object already deleted, skipping reconcile"
                        );
                    }
                    Err(err) => {
                        tracing::warn!(kind = "LoadBalancer", error = ?err, "Controller stream error (will retry)");
                    }
                }
            })
            .await;
    };

    // Run both controllers; if one exits, both stop
    tokio::select! {
        _ = vmnc_controller => { info!("VMNC controller stopped"); }
        _ = lb_controller => { info!("LoadBalancer controller stopped"); }
    }
}
