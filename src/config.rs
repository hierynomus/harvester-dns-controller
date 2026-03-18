use serde::Deserialize;

/// Operator configuration, sourced from environment variables via `envy`.
///
/// Example deployment env vars:
///
/// ```text
/// ROUTEROS_HOST=192.168.1.1
/// ROUTEROS_USERNAME=dns-operator
/// ROUTEROS_PASSWORD=secret
/// ROUTEROS_TLS_VERIFY=true          # set false for self-signed certs in lab
/// DNS_DOMAIN=lab.example.com
/// DNS_TTL=15m
/// DNS_COMMENT_TAG=managed-by=harvester-dns-controller
/// WATCH_NAMESPACES=default,harvester-system   # empty = all namespaces
/// ```
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// RouterOS host — IP or hostname
    #[serde(default = "default_routeros_host")]
    pub routeros_host: String,

    /// RouterOS REST API username
    #[serde(default = "default_routeros_username")]
    pub routeros_username: String,

    /// RouterOS REST API password
    pub routeros_password: String,

    /// Verify TLS certificate. Set to false for self-signed lab certs.
    #[serde(default = "default_tls_verify")]
    pub routeros_tls_verify: bool,

    /// Use HTTPS (true) or HTTP (false). HTTP only for local lab without certs.
    #[serde(default = "default_tls")]
    pub routeros_use_tls: bool,

    /// Domain suffix appended to the VM name, e.g. "lab.example.com"
    /// Result: <vm-name>.<domain>
    pub dns_domain: String,

    /// TTL for created DNS records, in RouterOS format, e.g. "15m", "1h", "1d"
    #[serde(default = "default_ttl")]
    pub dns_ttl: String,

    /// Comment tag written to every managed DNS record. Used to identify and
    /// clean up records owned by this operator.
    #[serde(default = "default_comment_tag")]
    pub dns_comment_tag: String,

    /// Comma-separated list of namespaces to watch.
    /// If empty, watch all namespaces.
    #[serde(default)]
    pub watch_namespaces: String,

    /// Whether to use the guestcluster.harvesterhci.io/name label to derive hostnames.
    /// When true, VMs belonging to a Rancher guest cluster use the cluster name as hostname.
    /// When false, always use the VM name (useful if guest cluster has its own DNS controller).
    #[serde(default = "default_use_guest_cluster_label")]
    pub dns_use_guest_cluster_label: bool,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        envy::from_env::<Config>().map_err(|e| anyhow::anyhow!("Config error: {}", e))
    }

    /// Build the base URL for the RouterOS REST API.
    pub fn routeros_base_url(&self) -> String {
        let scheme = if self.routeros_use_tls { "https" } else { "http" };
        format!("{}://{}/rest", scheme, self.routeros_host)
    }

    /// Build the FQDN for a VM.
    pub fn fqdn_for(&self, vm_name: &str) -> String {
        format!("{}.{}", vm_name.to_lowercase(), self.dns_domain)
    }

    /// Parse the watch_namespaces into a list. Empty string → watch all.
    pub fn namespaces(&self) -> Vec<String> {
        if self.watch_namespaces.is_empty() {
            vec![]
        } else {
            self.watch_namespaces
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }
    }
}

fn default_routeros_host() -> String {
    "192.168.1.1".to_string()
}

fn default_routeros_username() -> String {
    "admin".to_string()
}

fn default_tls_verify() -> bool {
    true
}

fn default_tls() -> bool {
    true
}

fn default_ttl() -> String {
    "15m".to_string()
}

fn default_comment_tag() -> String {
    "managed-by=harvester-dns-controller".to_string()
}

fn default_use_guest_cluster_label() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            routeros_host: "192.168.1.1".to_string(),
            routeros_username: "admin".to_string(),
            routeros_password: "secret".to_string(),
            routeros_tls_verify: true,
            routeros_use_tls: true,
            dns_domain: "lab.example.com".to_string(),
            dns_ttl: "15m".to_string(),
            dns_comment_tag: "managed-by=test".to_string(),
            watch_namespaces: "".to_string(),
            dns_use_guest_cluster_label: true,
        }
    }

    #[test]
    fn test_fqdn_for_lowercase() {
        let config = test_config();
        assert_eq!(config.fqdn_for("my-vm"), "my-vm.lab.example.com");
    }

    #[test]
    fn test_fqdn_for_uppercase_converted() {
        let config = test_config();
        assert_eq!(config.fqdn_for("MyVM"), "myvm.lab.example.com");
    }

    #[test]
    fn test_routeros_base_url_https() {
        let config = test_config();
        assert_eq!(config.routeros_base_url(), "https://192.168.1.1/rest");
    }

    #[test]
    fn test_routeros_base_url_http() {
        let mut config = test_config();
        config.routeros_use_tls = false;
        assert_eq!(config.routeros_base_url(), "http://192.168.1.1/rest");
    }

    #[test]
    fn test_namespaces_empty() {
        let config = test_config();
        assert!(config.namespaces().is_empty());
    }

    #[test]
    fn test_namespaces_single() {
        let mut config = test_config();
        config.watch_namespaces = "default".to_string();
        assert_eq!(config.namespaces(), vec!["default"]);
    }

    #[test]
    fn test_namespaces_multiple() {
        let mut config = test_config();
        config.watch_namespaces = "default, kube-system, harvester".to_string();
        assert_eq!(
            config.namespaces(),
            vec!["default", "kube-system", "harvester"]
        );
    }

    #[test]
    fn test_namespaces_with_empty_entries() {
        let mut config = test_config();
        config.watch_namespaces = "default,,kube-system, ".to_string();
        assert_eq!(config.namespaces(), vec!["default", "kube-system"]);
    }
}
