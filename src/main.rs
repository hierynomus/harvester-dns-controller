use std::sync::Arc;

use futures::StreamExt;
use kube::runtime::watcher;
use kube::runtime::Controller;
use kube::{Api, Client};
use tracing::info;

use harvester_dns_controller::{
    garbage_collect_on_startup, reconcile, error_policy,
    health, Config, Context, DnsClient, RouterOsClient, VmNetworkConfig,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise structured logging.
    // LOG_FORMAT=json for production; default is human-readable for local dev.
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_default();
    if log_format == "json" {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("harvester_dns_controller=info".parse()?)
                    .add_directive("warn".parse()?)
            )
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("harvester_dns_controller=info".parse()?)
                    .add_directive("warn".parse()?)
            )
            .init();
    }

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "harvester-dns-controller starting"
    );

    // Load operator config from environment variables
    let config = Config::from_env()?;
    info!(
        routeros_host = %config.routeros_host,
        dns_domain = %config.dns_domain,
        dns_ttl = %config.dns_ttl,
        comment_tag = %config.dns_comment_tag,
        "Configuration loaded"
    );

    // Build RouterOS REST API client
    let dns: Arc<dyn DnsClient> = Arc::new(RouterOsClient::new(&config)?);

    // Connect to Kubernetes (uses in-cluster config, falls back to KUBECONFIG)
    let kube_client = Client::try_default().await?;

    // Run startup garbage collection before the controller loop begins.
    // This cleans up any stale DNS records from VMs deleted while we were down.
    if let Err(e) = garbage_collect_on_startup(&kube_client, dns.as_ref(), &config).await {
        // Log but don't abort — the controller will self-correct over time.
        tracing::warn!(error = %e, "Startup garbage collection failed, continuing");
    }

    // Shared context passed into every reconcile call
    let ctx = Arc::new(Context {
        config,
        dns,
        kube: kube_client.clone(),
    });

    // Watch VirtualMachineNetworkConfig across all namespaces.
    // On Harvester, VMNCs live in the same namespace as the VirtualMachine
    // (typically "default" or a project namespace), so watching all is correct.
    let api: Api<VmNetworkConfig> = Api::all(kube_client.clone());

    // Start the HTTP health endpoint in the background so liveness/readiness
    // probes work from the moment the controller loop is running.
    let health_port = std::env::var("HEALTH_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8080);

    tokio::spawn(health::serve(health_port));
    info!(port = health_port, "Health endpoint listening");

    info!("Starting controller loop");

    // Configure watcher with reasonable timeouts and backoff
    let watcher_config = watcher::Config::default()
        .any_semantic();  // Use any semantic to handle both List and Watch modes gracefully

    Controller::new(api, watcher_config)
        .run(reconcile, error_policy, ctx)
        .for_each(|result| async move {
            match result {
                Ok((obj, action)) => {
                    info!(
                        name = %obj.name,
                        namespace = %obj.namespace.as_deref().unwrap_or(""),
                        requeue_after = ?action,
                        "Reconcile OK"
                    );
                }
                Err(err) => {
                    // Use Debug formatting to get full error chain
                    tracing::warn!(error = ?err, "Controller stream error (will retry)");
                }
            }
        })
        .await;

    Ok(())
}
