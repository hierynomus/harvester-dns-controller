//! Async client for the GL.Inet router REST API.
//!
//! Targets the custom DNS hosts feature available on GL.Inet routers
//! like the Beryl AX (GL-MT3000).

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::types::{GlInetDnsHost, ListHostsResponse, LoginResponse};
use crate::config::Config;
use crate::dns::DnsClient;

/// Async client for GL.Inet router DNS management.
///
/// Uses token-based authentication with automatic token refresh on 401 responses.
pub struct GlInetClient {
    http: Client,
    base_url: String,
    password: String,
    /// Cached auth token, refreshed on 401
    token: Arc<RwLock<Option<String>>>,
}

impl GlInetClient {
    pub fn new(config: &Config) -> Result<Self> {
        let http = Client::builder()
            .danger_accept_invalid_certs(!config.dns_tls_verify)
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            http,
            base_url: config.dns_base_url(),
            password: config.dns_password.clone(),
            token: Arc::new(RwLock::new(None)),
        })
    }

    /// Authenticate and obtain a new token.
    async fn login(&self) -> Result<String> {
        let url = format!("{}/cgi-bin/api/router/login", self.base_url);
        debug!(url = %url, "Authenticating with GL.Inet router");

        let resp = self
            .http
            .post(&url)
            .json(&json!({ "pwd": self.password }))
            .send()
            .await
            .context("POST /router/login failed")?;

        let status = resp.status();
        let body = resp.text().await.context("Failed to read login response")?;

        if !status.is_success() {
            anyhow::bail!("GL.Inet login failed with {}: {}", status, body);
        }

        let login_resp: LoginResponse =
            serde_json::from_str(&body).context("Failed to parse login response")?;

        match login_resp.token {
            Some(token) if !token.is_empty() => {
                debug!("Successfully authenticated with GL.Inet router");
                Ok(token)
            }
            _ => {
                anyhow::bail!(
                    "GL.Inet login failed: code={}, msg={}",
                    login_resp.code,
                    login_resp.msg
                );
            }
        }
    }

    /// Get a valid auth token, logging in if necessary.
    async fn get_token(&self) -> Result<String> {
        // Check cached token
        {
            let cached = self.token.read().await;
            if let Some(ref token) = *cached {
                return Ok(token.clone());
            }
        }

        // Need to login
        let token = self.login().await?;
        {
            let mut cached = self.token.write().await;
            *cached = Some(token.clone());
        }
        Ok(token)
    }

    /// Clear cached token (called on 401).
    async fn clear_token(&self) {
        let mut cached = self.token.write().await;
        *cached = None;
    }

    /// List all custom DNS hosts.
    pub async fn list_hosts(&self) -> Result<Vec<GlInetDnsHost>> {
        let url = format!("{}/cgi-bin/api/dns/custom_hosts", self.base_url);
        let token = self.get_token().await?;

        let resp = self
            .http
            .get(&url)
            .header("Authorization", &token)
            .send()
            .await
            .context("GET /dns/custom_hosts failed")?;

        let status = resp.status();

        if status == StatusCode::UNAUTHORIZED {
            self.clear_token().await;
            anyhow::bail!("GL.Inet auth token expired");
        }

        let body = resp.text().await.context("Failed to read response")?;

        if !status.is_success() {
            anyhow::bail!("GL.Inet returned {}: {}", status, body);
        }

        let list_resp: ListHostsResponse =
            serde_json::from_str(&body).context("Failed to parse hosts list")?;

        debug!(count = list_resp.hosts.len(), "Found DNS hosts");
        Ok(list_resp.hosts)
    }

    /// List only hosts managed by this operator (domain starts with comment tag pattern).
    ///
    /// GL.Inet doesn't have a comment field, so we use a suffix convention:
    /// managed hosts end with a marker like `.harvester-dns-controller`.
    /// Actually, for simplicity, we'll track ALL hosts we create.
    pub async fn list_managed_hosts(&self) -> Result<Vec<GlInetDnsHost>> {
        // Since GL.Inet doesn't support comments/tags, we'll manage all hosts
        // that match domains we're responsible for. The caller (garbage_collect)
        // should pass the list of expected FQDNs.
        self.list_hosts().await
    }

    /// Add a custom DNS host.
    async fn add_host(&self, domain: &str, ip: &str) -> Result<()> {
        let url = format!("{}/cgi-bin/api/dns/custom_hosts", self.base_url);
        let token = self.get_token().await?;

        let payload = json!({
            "domain": domain,
            "ip": ip,
            "enabled": true
        });

        let resp = self
            .http
            .post(&url)
            .header("Authorization", &token)
            .json(&payload)
            .send()
            .await
            .context("POST /dns/custom_hosts failed")?;

        let status = resp.status();

        if status == StatusCode::UNAUTHORIZED {
            self.clear_token().await;
            anyhow::bail!("GL.Inet auth token expired");
        }

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("GL.Inet returned {} adding host: {}", status, body);
        }

        Ok(())
    }

    /// Update an existing host (delete + add since GL.Inet may not support PATCH).
    async fn update_host(&self, domain: &str, ip: &str) -> Result<()> {
        // GL.Inet API typically requires delete + add for updates
        self.delete_host(domain).await?;
        self.add_host(domain, ip).await
    }

    /// Delete a custom DNS host by domain.
    async fn delete_host(&self, domain: &str) -> Result<()> {
        let url = format!("{}/cgi-bin/api/dns/custom_hosts", self.base_url);
        let token = self.get_token().await?;

        let payload = json!({ "domain": domain });

        let resp = self
            .http
            .delete(&url)
            .header("Authorization", &token)
            .json(&payload)
            .send()
            .await
            .context("DELETE /dns/custom_hosts failed")?;

        let status = resp.status();

        if status == StatusCode::UNAUTHORIZED {
            self.clear_token().await;
            anyhow::bail!("GL.Inet auth token expired");
        }

        // 404 is acceptable - host may already be deleted
        if !status.is_success() && status != StatusCode::NOT_FOUND {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("GL.Inet returned {} deleting host: {}", status, body);
        }

        Ok(())
    }

    /// Find a host by domain name.
    async fn find_host(&self, domain: &str) -> Result<Option<GlInetDnsHost>> {
        let hosts = self.list_hosts().await?;
        Ok(hosts.into_iter().find(|h| h.domain == domain))
    }
}

// ---------------------------------------------------------------------------
// DnsClient trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl DnsClient for GlInetClient {
    async fn ensure_record(&self, fqdn: &str, ip: &str, _ttl: &str) -> Result<()> {
        // GL.Inet doesn't support TTL for custom hosts, so we ignore it

        match self.find_host(fqdn).await? {
            Some(host) if host.ip == ip => {
                debug!(fqdn = %fqdn, ip = %ip, "DNS host already up to date");
                Ok(())
            }
            Some(_) => {
                info!(fqdn = %fqdn, ip = %ip, "Updating DNS host");
                self.update_host(fqdn, ip).await
            }
            None => {
                info!(fqdn = %fqdn, ip = %ip, "Creating DNS host");
                self.add_host(fqdn, ip).await
            }
        }
    }

    async fn delete_record_for_fqdn(&self, fqdn: &str) -> Result<()> {
        match self.find_host(fqdn).await? {
            Some(_) => {
                info!(fqdn = %fqdn, "Deleting DNS host");
                self.delete_host(fqdn).await
            }
            None => {
                debug!(fqdn = %fqdn, "No DNS host found to delete");
                Ok(())
            }
        }
    }

    async fn garbage_collect(&self, active_fqdns: &[String]) -> Result<()> {
        // Without a comment tag, we can only GC hosts that match our known FQDNs
        // This means we need to track what we've created elsewhere
        // For now, we'll list all hosts and remove those not in active_fqdns
        // that we might have created (this is a limitation)

        let all_hosts = self.list_hosts().await?;
        for host in all_hosts {
            // Only clean up if this looks like a domain we manage
            // (e.g., matches our DNS suffix pattern)
            if !active_fqdns.contains(&host.domain) {
                // Note: This is conservative - we only delete if we're sure
                // For safety, we skip GC for GL.Inet as we can't identify our records
                debug!(
                    domain = %host.domain,
                    "Skipping potential stale host (cannot confirm ownership)"
                );
            }
        }

        warn!(
            "GL.Inet garbage collection is limited - cannot identify managed records without comment tags"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Integration tests would require a GL.Inet router
}
