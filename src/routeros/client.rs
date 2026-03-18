//! Thin async client for the RouterOS REST API.
//!
//! Scoped to the /ip/dns/static endpoint, which is all we need.

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use tracing::{debug, info, warn};

use super::types::RouterOsDnsRecord;
use crate::config::Config;
use crate::dns::DnsClient;

/// Thin async client for the RouterOS REST API.
/// Scoped to the /ip/dns/static endpoint, which is all we need.
pub struct RouterOsClient {
    http: Client,
    base_url: String,
    username: String,
    password: String,
    comment_tag: String,
}

impl RouterOsClient {
    pub fn new(config: &Config) -> Result<Self> {
        let http = Client::builder()
            .danger_accept_invalid_certs(!config.routeros_tls_verify)
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            http,
            base_url: config.routeros_base_url(),
            username: config.routeros_username.clone(),
            password: config.routeros_password.clone(),
            comment_tag: config.dns_comment_tag.clone(),
        })
    }

    /// List all DNS static records managed by this operator (filtered by comment tag).
    pub async fn list_managed_records(&self) -> Result<Vec<RouterOsDnsRecord>> {
        let url = format!("{}/ip/dns/static", self.base_url);
        debug!(url = %url, "Listing DNS static records");

        let resp = self
            .http
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .context("GET /ip/dns/static failed")?;

        let status = resp.status();
        let body = resp.text().await.context("Failed to read response body")?;

        if !status.is_success() {
            anyhow::bail!("RouterOS returned {}: {}", status, body);
        }

        let records: Vec<RouterOsDnsRecord> =
            serde_json::from_str(&body).context("Failed to parse DNS record list")?;

        // Filter to only records this operator created
        let managed: Vec<_> = records
            .into_iter()
            .filter(|r| r.comment == self.comment_tag)
            .collect();

        debug!(count = managed.len(), "Found managed DNS records");
        Ok(managed)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Find any record by name, regardless of comment tag.
    async fn find_any_record_by_name(&self, name: &str) -> Result<Option<RouterOsDnsRecord>> {
        let url = format!(
            "{}/ip/dns/static?name={}",
            self.base_url,
            urlencoding::encode(name),
        );

        let resp = self
            .http
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .context("GET /ip/dns/static (by name) failed")?;

        let status = resp.status();

        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let body = resp.text().await.context("Failed to read response body")?;
        if !status.is_success() {
            anyhow::bail!("RouterOS returned {}: {}", status, body);
        }

        let records: Vec<RouterOsDnsRecord> =
            serde_json::from_str(&body).context("Failed to parse DNS record")?;

        Ok(records.into_iter().next())
    }

    /// Find a record by name that is managed by this operator (has our comment tag).
    async fn find_record_by_name(&self, name: &str) -> Result<Option<RouterOsDnsRecord>> {
        // RouterOS REST API supports query params for filtering
        let url = format!(
            "{}/ip/dns/static?name={}&comment={}",
            self.base_url,
            urlencoding::encode(name),
            urlencoding::encode(&self.comment_tag)
        );

        let resp = self
            .http
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .context("GET /ip/dns/static (filtered) failed")?;

        let status = resp.status();

        // RouterOS returns 404 when the filter matches nothing in some versions
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let body = resp.text().await.context("Failed to read response body")?;
        if !status.is_success() {
            anyhow::bail!("RouterOS returned {}: {}", status, body);
        }

        // The response is always an array, even for a single result
        let records: Vec<RouterOsDnsRecord> =
            serde_json::from_str(&body).context("Failed to parse DNS record")?;

        Ok(records.into_iter().next())
    }

    async fn create_record(&self, name: &str, address: &str, ttl: &str) -> Result<()> {
        let url = format!("{}/ip/dns/static", self.base_url);
        let payload = RouterOsDnsRecord {
            name: name.to_string(),
            address: address.to_string(),
            ttl: ttl.to_string(),
            comment: self.comment_tag.clone(),
            match_subdomain: true,
            ..Default::default()
        };

        let resp = self
            .http
            .put(&url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&payload)
            .send()
            .await
            .context("PUT /ip/dns/static failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("RouterOS returned {} creating record: {}", status, body);
        }

        Ok(())
    }

    async fn update_record(&self, id: &str, name: &str, address: &str, ttl: &str) -> Result<()> {
        // RouterOS REST: PATCH /{path}/{.id} to update
        let url = format!("{}/ip/dns/static/{}", self.base_url, id);
        let payload = RouterOsDnsRecord {
            name: name.to_string(),
            address: address.to_string(),
            ttl: ttl.to_string(),
            comment: self.comment_tag.clone(),
            match_subdomain: true,
            ..Default::default()
        };

        let resp = self
            .http
            .patch(&url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&payload)
            .send()
            .await
            .context("PATCH /ip/dns/static failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("RouterOS returned {} updating record: {}", status, body);
        }

        Ok(())
    }

    async fn delete_record(&self, id: &str) -> Result<()> {
        let url = format!("{}/ip/dns/static/{}", self.base_url, id);

        let resp = self
            .http
            .delete(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .context("DELETE /ip/dns/static failed")?;

        let status = resp.status();
        if !status.is_success() && status != StatusCode::NOT_FOUND {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("RouterOS returned {} deleting record: {}", status, body);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DnsClient trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl DnsClient for RouterOsClient {
    async fn ensure_record(&self, fqdn: &str, ip: &str, ttl: &str) -> Result<()> {
        // Check for ANY record with this name (regardless of who owns it)
        let existing = self.find_any_record_by_name(fqdn).await?;

        match existing {
            Some(record) if record.address == ip && record.comment == self.comment_tag => {
                debug!(fqdn = %fqdn, ip = %ip, "DNS record already up to date");
                Ok(())
            }
            Some(record) if record.comment == self.comment_tag => {
                // Owned by us but IP changed
                info!(fqdn = %fqdn, old_ip = %record.address, new_ip = %ip, "Updating DNS record");
                self.update_record(&record.id, fqdn, ip, ttl).await
            }
            Some(record) => {
                // Record exists but not owned by us — take ownership
                warn!(
                    fqdn = %fqdn,
                    old_comment = %record.comment,
                    "Taking ownership of existing DNS record"
                );
                self.update_record(&record.id, fqdn, ip, ttl).await
            }
            None => {
                info!(fqdn = %fqdn, ip = %ip, "Creating DNS record");
                self.create_record(fqdn, ip, ttl).await
            }
        }
    }

    async fn delete_record_for_fqdn(&self, fqdn: &str) -> Result<()> {
        match self.find_record_by_name(fqdn).await? {
            Some(record) => {
                info!(fqdn = %fqdn, id = %record.id, "Deleting DNS record");
                self.delete_record(&record.id).await
            }
            None => {
                debug!(fqdn = %fqdn, "No DNS record found to delete");
                Ok(())
            }
        }
    }

    async fn garbage_collect(&self, active_fqdns: &[String]) -> Result<()> {
        let managed = self.list_managed_records().await?;
        for record in managed {
            if !active_fqdns.contains(&record.name) {
                warn!(
                    fqdn = %record.name,
                    id = %record.id,
                    "Removing stale DNS record (VM no longer exists)"
                );
                self.delete_record(&record.id).await?;
            }
        }
        Ok(())
    }
}

// Inline the urlencoding helper rather than pulling in an extra crate
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(c),
                c => {
                    for byte in c.to_string().as_bytes() {
                        out.push('%');
                        out.push_str(&format!("{:02X}", byte));
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::urlencoding;

    #[test]
    fn test_urlencoding_passthrough() {
        // Alphanumeric and safe chars should pass through
        assert_eq!(urlencoding::encode("hello"), "hello");
        assert_eq!(urlencoding::encode("Hello123"), "Hello123");
        assert_eq!(urlencoding::encode("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn test_urlencoding_spaces() {
        assert_eq!(urlencoding::encode("hello world"), "hello%20world");
    }

    #[test]
    fn test_urlencoding_special_chars() {
        assert_eq!(urlencoding::encode("a=b"), "a%3Db");
        assert_eq!(urlencoding::encode("a&b"), "a%26b");
        assert_eq!(urlencoding::encode("a+b"), "a%2Bb");
    }

    #[test]
    fn test_urlencoding_fqdn() {
        // FQDNs should mostly pass through (dots and hyphens are safe)
        assert_eq!(
            urlencoding::encode("myvm.lab.example.com"),
            "myvm.lab.example.com"
        );
    }

    #[test]
    fn test_urlencoding_comment_tag() {
        // Comment tags typically have = which needs encoding
        assert_eq!(
            urlencoding::encode("managed-by=test"),
            "managed-by%3Dtest"
        );
    }
}
