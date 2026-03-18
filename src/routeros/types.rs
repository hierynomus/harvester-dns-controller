//! RouterOS REST API response and request types.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Deserialize a RouterOS boolean, which comes as a string "true" or "false".
fn deserialize_routeros_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    Ok(s.map(|v| v == "true").unwrap_or(false))
}

/// Serialize a bool as a RouterOS string "true" or "false".
fn serialize_routeros_bool<S>(value: &bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(if *value { "true" } else { "false" })
}

/// A DNS static record for RouterOS REST API.
/// Used for both reading from and writing to the API.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RouterOsDnsRecord {
    /// RouterOS internal ID, e.g. "*1A". Read-only.
    #[serde(rename = ".id", default, skip_serializing)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub ttl: String,
    #[serde(default)]
    pub comment: String,
    /// Read-only field from API responses.
    #[serde(default, skip_serializing)]
    pub disabled: String,
    /// Read-only field from API responses.
    #[serde(rename = "type", default, skip_serializing)]
    pub record_type: String,
    /// When true, this record also matches all subdomains.
    #[serde(
        default,
        skip_serializing_if = "std::ops::Not::not",
        deserialize_with = "deserialize_routeros_bool",
        serialize_with = "serialize_routeros_bool"
    )]
    pub match_subdomain: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_dns_record() {
        let json = r#"{
            ".id": "*1A",
            "name": "myvm.lab.example.com",
            "address": "192.168.1.100",
            "ttl": "15m",
            "comment": "managed-by=test",
            "disabled": "false",
            "type": "A",
            "match-subdomain": "true"
        }"#;

        let record: RouterOsDnsRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.id, "*1A");
        assert_eq!(record.name, "myvm.lab.example.com");
        assert_eq!(record.address, "192.168.1.100");
        assert_eq!(record.ttl, "15m");
        assert_eq!(record.comment, "managed-by=test");
        assert_eq!(record.disabled, "false");
        assert_eq!(record.record_type, "A");
        assert!(record.match_subdomain);
    }

    #[test]
    fn test_deserialize_dns_record_minimal() {
        // RouterOS may omit some fields
        let json = r#"{
            "name": "myvm.lab.example.com"
        }"#;

        let record: RouterOsDnsRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.name, "myvm.lab.example.com");
        assert_eq!(record.id, "");
        assert_eq!(record.address, "");
        assert!(!record.match_subdomain);
    }


    #[test]
    fn test_serialize_dns_record_non_serialized_fields() {
        let payload = RouterOsDnsRecord {
            name: "myvm.lab.example.com".to_string(),
            address: "192.168.1.100".to_string(),
            ttl: "15m".to_string(),
            comment: "managed-by=test".to_string(),
            match_subdomain: true,
            // These fields should not be serialized
            id: "*1A".to_string(),
            disabled: "false".to_string(),
            record_type: "A".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_value(&payload).unwrap();
        // Read-only fields should not be serialized
        assert!(json.get(".id").is_none());
        assert!(json.get("disabled").is_none());
        assert!(json.get("type").is_none());
    }


    #[test]
    fn test_serialize_dns_record() {
        let payload = RouterOsDnsRecord {
            name: "myvm.lab.example.com".to_string(),
            address: "192.168.1.100".to_string(),
            ttl: "15m".to_string(),
            comment: "managed-by=test".to_string(),
            match_subdomain: true,
            ..Default::default()
        };

        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["name"], "myvm.lab.example.com");
        assert_eq!(json["address"], "192.168.1.100");
        assert_eq!(json["ttl"], "15m");
        assert_eq!(json["comment"], "managed-by=test");
        // RouterOS expects boolean as string "true"
        assert_eq!(json["match-subdomain"], "true");
        // Read-only fields should not be serialized
        assert!(json.get(".id").is_none());
        assert!(json.get("disabled").is_none());
        assert!(json.get("type").is_none());
    }

    #[test]
    fn test_serialize_dns_record_no_match_subdomain() {
        let payload = RouterOsDnsRecord {
            name: "myvm.lab.example.com".to_string(),
            address: "192.168.1.100".to_string(),
            ttl: "15m".to_string(),
            comment: "managed-by=test".to_string(),
            match_subdomain: false,
            ..Default::default()
        };

        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["name"], "myvm.lab.example.com");
        // match-subdomain should be omitted when false
        assert!(json.get("match-subdomain").is_none());
    }

    #[test]
    fn test_deserialize_dns_record_array() {
        let json = r#"[
            {".id": "*1", "name": "vm1.example.com", "address": "10.0.0.1"},
            {".id": "*2", "name": "vm2.example.com", "address": "10.0.0.2"}
        ]"#;

        let records: Vec<RouterOsDnsRecord> = serde_json::from_str(json).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, "*1");
        assert_eq!(records[1].id, "*2");
    }
}
