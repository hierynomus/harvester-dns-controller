use serde::Deserialize;
use std::fmt;
use std::str::FromStr;

/// Supported DNS backends.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DnsBackend {
    #[default]
    RouterOs,
    GlInet,
}

impl fmt::Display for DnsBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DnsBackend::RouterOs => write!(f, "routeros"),
            DnsBackend::GlInet => write!(f, "glinet"),
        }
    }
}

impl FromStr for DnsBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "routeros" | "ros" | "mikrotik" => Ok(DnsBackend::RouterOs),
            "glinet" | "gl-inet" | "gl.inet" => Ok(DnsBackend::GlInet),
            _ => Err(format!("Unknown DNS backend: {}", s)),
        }
    }
}

impl<'de> Deserialize<'de> for DnsBackend {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DnsBackend::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Operator configuration, sourced from environment variables via `envy`.
///
/// Example deployment env vars:
///
/// ```text
/// DNS_BACKEND=routeros   # or 'glinet'
/// DNS_HOST=192.168.1.1
/// DNS_USERNAME=dns-operator   # RouterOS only, ignored by GL.Inet
/// DNS_PASSWORD=secret
/// DNS_USE_TLS=true
/// DNS_TLS_VERIFY=true          # set false for self-signed certs in lab
/// DNS_DOMAIN=lab.example.com
/// DNS_TTL=15m
/// DNS_COMMENT_TAG=managed-by=harvester-dns-controller
/// WATCH_NAMESPACES=default,harvester-system   # empty = all namespaces
/// ```
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// DNS backend to use: `routeros` or `glinet`.
    #[serde(default)]
    pub dns_backend: DnsBackend,

    /// Router host — IP or hostname.
    #[serde(default = "default_dns_host")]
    pub dns_host: String,

    /// API username (RouterOS only, ignored by GL.Inet).
    #[serde(default = "default_dns_username")]
    pub dns_username: String,

    /// API/admin password.
    #[serde(default)]
    pub dns_password: String,

    /// Use HTTPS (true) or HTTP (false).
    #[serde(default = "default_tls")]
    pub dns_use_tls: bool,

    /// Verify TLS certificate. Set to false for self-signed lab certs.
    #[serde(default = "default_tls_verify")]
    pub dns_tls_verify: bool,

    /// Domain suffix appended to the VM name, e.g. "lab.example.com".
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

    /// Build the base URL for the DNS backend API.
    pub fn dns_base_url(&self) -> String {
        let scheme = if self.dns_use_tls { "https" } else { "http" };
        match self.dns_backend {
            DnsBackend::RouterOs => format!("{}://{}/rest", scheme, self.dns_host),
            DnsBackend::GlInet => format!("{}://{}", scheme, self.dns_host),
        }
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

fn default_dns_host() -> String {
    "192.168.1.1".to_string()
}

fn default_dns_username() -> String {
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
            dns_backend: DnsBackend::RouterOs,
            dns_host: "192.168.1.1".to_string(),
            dns_username: "admin".to_string(),
            dns_password: "secret".to_string(),
            dns_use_tls: true,
            dns_tls_verify: true,
            dns_domain: "lab.example.com".to_string(),
            dns_ttl: "15m".to_string(),
            dns_comment_tag: "managed-by=test".to_string(),
            watch_namespaces: "".to_string(),
            dns_use_guest_cluster_label: true,
        }
    }

    #[test]
    fn test_dns_backend_from_str() {
        assert_eq!("routeros".parse::<DnsBackend>().unwrap(), DnsBackend::RouterOs);
        assert_eq!("ros".parse::<DnsBackend>().unwrap(), DnsBackend::RouterOs);
        assert_eq!("mikrotik".parse::<DnsBackend>().unwrap(), DnsBackend::RouterOs);
        assert_eq!("glinet".parse::<DnsBackend>().unwrap(), DnsBackend::GlInet);
        assert_eq!("gl-inet".parse::<DnsBackend>().unwrap(), DnsBackend::GlInet);
        assert!("invalid".parse::<DnsBackend>().is_err());
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
    fn test_dns_base_url_routeros_https() {
        let config = test_config();
        assert_eq!(config.dns_base_url(), "https://192.168.1.1/rest");
    }

    #[test]
    fn test_dns_base_url_routeros_http() {
        let mut config = test_config();
        config.dns_use_tls = false;
        assert_eq!(config.dns_base_url(), "http://192.168.1.1/rest");
    }

    #[test]
    fn test_dns_base_url_glinet_https() {
        let mut config = test_config();
        config.dns_backend = DnsBackend::GlInet;
        config.dns_use_tls = true;
        assert_eq!(config.dns_base_url(), "https://192.168.1.1");
    }

    #[test]
    fn test_dns_base_url_glinet_http() {
        let mut config = test_config();
        config.dns_backend = DnsBackend::GlInet;
        config.dns_use_tls = false;
        assert_eq!(config.dns_base_url(), "http://192.168.1.1");
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
