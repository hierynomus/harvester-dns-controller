//! RouterOS REST API response and request types.

use serde::{Deserialize, Serialize};

/// A DNS static record as returned by GET /rest/ip/dns/static
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterOsDnsRecord {
    /// RouterOS internal ID, e.g. "*1A"
    #[serde(rename = ".id", default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub ttl: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub disabled: String,
    #[serde(rename = "type", default)]
    pub record_type: String,
}

/// Payload for creating or updating a DNS static record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterOsDnsRecordPut {
    pub name: String,
    pub address: String,
    pub ttl: String,
    pub comment: String,
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
            "type": "A"
        }"#;

        let record: RouterOsDnsRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.id, "*1A");
        assert_eq!(record.name, "myvm.lab.example.com");
        assert_eq!(record.address, "192.168.1.100");
        assert_eq!(record.ttl, "15m");
        assert_eq!(record.comment, "managed-by=test");
        assert_eq!(record.disabled, "false");
        assert_eq!(record.record_type, "A");
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
    }

    #[test]
    fn test_serialize_dns_record_put() {
        let payload = RouterOsDnsRecordPut {
            name: "myvm.lab.example.com".to_string(),
            address: "192.168.1.100".to_string(),
            ttl: "15m".to_string(),
            comment: "managed-by=test".to_string(),
        };

        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["name"], "myvm.lab.example.com");
        assert_eq!(json["address"], "192.168.1.100");
        assert_eq!(json["ttl"], "15m");
        assert_eq!(json["comment"], "managed-by=test");
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
