//! Types for GL.Inet DNS custom hosts.

use serde::{Deserialize, Serialize};

/// Response from GL.Inet login endpoint.
#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    pub token: Option<String>,
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub msg: String,
}

/// A custom DNS host entry in GL.Inet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlInetDnsHost {
    /// Hostname (FQDN)
    pub domain: String,
    /// Target IP address
    pub ip: String,
    /// Whether the entry is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// Response from list hosts endpoint.
#[derive(Debug, Deserialize)]
pub struct ListHostsResponse {
    #[serde(default)]
    pub hosts: Vec<GlInetDnsHost>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_login_response() {
        let json = r#"{"token": "abc123", "code": 0}"#;
        let resp: LoginResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.token, Some("abc123".to_string()));
        assert_eq!(resp.code, 0);
    }

    #[test]
    fn test_deserialize_host() {
        let json = r#"{"domain": "test.local", "ip": "192.168.1.1", "enabled": true}"#;
        let host: GlInetDnsHost = serde_json::from_str(json).unwrap();
        assert_eq!(host.domain, "test.local");
        assert_eq!(host.ip, "192.168.1.1");
        assert!(host.enabled);
    }

    #[test]
    fn test_serialize_host() {
        let host = GlInetDnsHost {
            domain: "test.local".to_string(),
            ip: "10.0.0.1".to_string(),
            enabled: true,
        };
        let json = serde_json::to_string(&host).unwrap();
        assert!(json.contains(r#""domain":"test.local""#));
        assert!(json.contains(r#""ip":"10.0.0.1""#));
    }
}
