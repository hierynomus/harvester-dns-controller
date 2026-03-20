use std::sync::Arc;

use kube::Client;
use tracing::info;

use harvester_dns_controller::{
    garbage_collect_on_startup, run_controllers, health,
    Config, Context, DnsBackend, DnsClient, GlInetClient, HostnameRegistry, RouterOsClient,
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
        dns_backend = %config.dns_backend,
        dns_domain = %config.dns_domain,
        dns_ttl = %config.dns_ttl,
        comment_tag = %config.dns_comment_tag,
        use_guest_cluster_label = %config.dns_use_guest_cluster_label,
        "Configuration loaded"
    );

    // Build DNS client based on configured backend
    let dns: Arc<dyn DnsClient> = match config.dns_backend {
        DnsBackend::RouterOs => {
            info!(host = %config.dns_host, "Using RouterOS DNS backend");
            Arc::new(RouterOsClient::new(&config)?)
        }
        DnsBackend::GlInet => {
            info!(host = %config.dns_host, "Using GL.Inet DNS backend");
            Arc::new(GlInetClient::new(&config)?)
        }
    };

    let registry = Arc::new(HostnameRegistry::new(
        dns.clone(),
        config.dns_domain.clone(),
        config.dns_ttl.clone(),
    ));

    // Connect to Kubernetes (uses in-cluster config, falls back to KUBECONFIG)
    let kube_client = Client::try_default().await?;

    // Run startup garbage collection before the controller loop begins.
    // This populates the registry from existing VMNCs and LBs, then removes stale DNS records.
    if let Err(e) = garbage_collect_on_startup(&kube_client, &registry, &config).await {
        // Log but don't abort — the controller will self-correct over time.
        tracing::warn!(error = %e, "Startup garbage collection failed, continuing");
    }

    // Start the HTTP health endpoint in the background so liveness/readiness
    // probes work from the moment the controller loop is running.
    let health_port = std::env::var("HEALTH_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8080);

    tokio::spawn(health::serve(health_port));
    info!(port = health_port, "Health endpoint listening");

    info!("Starting controllers");

    // Create shared context for controllers
    let ctx = Arc::new(Context {
        config,
        registry,
        kube: kube_client,
    });

    // Run both VMNC and LoadBalancer controllers concurrently
    run_controllers(ctx).await;

    Ok(())
}
