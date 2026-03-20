//! Types for RouterOS DNS static records.

use serde::{Deserialize, Serialize};

/// Represents a RouterOS DNS static record.
///
/// RouterOS uses string-encoded booleans ("true"/"false") for some fields,
/// hence the custom serialization helpers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RouterOsDnsRecord {
    #[serde(rename = ".id", skip_serializing, default)]
    pub id: String,

    pub name: String,
    pub address: String,

    #[serde(default)]
    pub ttl: String,

    #[serde(default)]
    pub comment: String,

    #[serde(
        rename = "match-subdomain",
        default,
        deserialize_with = "routeros_bool::deserialize",
        serialize_with = "routeros_bool::serialize"
    )]
    pub match_subdomain: bool,

    #[serde(
        default,
        deserialize_with = "routeros_bool::deserialize",
        serialize_with = "routeros_bool::serialize"
    )]
    pub disabled: bool,
}

/// Custom serde helpers for RouterOS string-encoded booleans.
///
/// RouterOS returns booleans as the literal strings "true" and "false",
/// and accepts the same format on write. This module provides symmetric
/// serialization and deserialization.
mod routeros_bool {
    use serde::{Deserialize, Deserializer, Serializer};

    /// Deserialize a RouterOS boolean from a string.
    ///
    /// Accepts "true", "false", or actual JSON booleans (for robustness).
    pub fn deserialize<'de, D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrBool {
            String(String),
            Bool(bool),
        }

        match StringOrBool::deserialize(deserializer)? {
            StringOrBool::String(s) => Ok(s.eq_ignore_ascii_case("true")),
            StringOrBool::Bool(b) => Ok(b),
        }
    }

    /// Serialize a boolean to a RouterOS string.
    pub fn serialize<S>(value: &bool, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(if *value { "true" } else { "false" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deserialize_basic() {
        let json = json!({
            ".id": "*1",
            "name": "test.example.com",
            "address": "192.168.1.1",
            "ttl": "1h",
            "comment": "harvester-dns-controller",
            "match-subdomain": "false",
            "disabled": "false"
        });

        let record: RouterOsDnsRecord = serde_json::from_value(json).unwrap();
        assert_eq!(record.id, "*1");
        assert_eq!(record.name, "test.example.com");
        assert_eq!(record.address, "192.168.1.1");
        assert_eq!(record.ttl, "1h");
        assert_eq!(record.comment, "harvester-dns-controller");
        assert!(!record.match_subdomain);
        assert!(!record.disabled);
    }

    #[test]
    fn test_deserialize_match_subdomain_true() {
        let json = json!({
            ".id": "*2",
            "name": "wildcard.example.com",
            "address": "192.168.1.2",
            "match-subdomain": "true",
            "disabled": "false"
        });

        let record: RouterOsDnsRecord = serde_json::from_value(json).unwrap();
        assert!(record.match_subdomain);
    }

    #[test]
    fn test_deserialize_disabled_true() {
        let json = json!({
            ".id": "*3",
            "name": "disabled.example.com",
            "address": "192.168.1.3",
            "match-subdomain": "false",
            "disabled": "true"
        });

        let record: RouterOsDnsRecord = serde_json::from_value(json).unwrap();
        assert!(record.disabled);
    }

    #[test]
    fn test_deserialize_minimal() {
        // RouterOS may omit optional fields
        let json = json!({
            "name": "minimal.example.com",
            "address": "10.0.0.1"
        });

        let record: RouterOsDnsRecord = serde_json::from_value(json).unwrap();
        assert_eq!(record.name, "minimal.example.com");
        assert_eq!(record.address, "10.0.0.1");
        assert!(record.id.is_empty());
        assert!(record.ttl.is_empty());
        assert!(record.comment.is_empty());
        assert!(!record.match_subdomain);
        assert!(!record.disabled);
    }

    #[test]
    fn test_serialize_roundtrip() {
        let record = RouterOsDnsRecord {
            id: "*123".to_string(), // id should be skipped
            name: "roundtrip.example.com".to_string(),
            address: "172.16.0.1".to_string(),
            ttl: "30m".to_string(),
            comment: "test-comment".to_string(),
            match_subdomain: true,
            disabled: false,
        };

        let json_str = serde_json::to_string(&record).unwrap();

        // Verify id is not serialized
        assert!(!json_str.contains(".id"));
        assert!(!json_str.contains("*123"));

        // Verify booleans are serialized as strings
        assert!(json_str.contains(r#""match-subdomain":"true""#));
        assert!(json_str.contains(r#""disabled":"false""#));
    }

    #[test]
    fn test_deserialize_json_booleans() {
        // Should also accept actual JSON booleans for robustness
        let json = json!({
            "name": "bool-test.example.com",
            "address": "192.168.1.1",
            "match-subdomain": true,
            "disabled": false
        });

        let record: RouterOsDnsRecord = serde_json::from_value(json).unwrap();
        assert!(record.match_subdomain);
        assert!(!record.disabled);
    }
}
