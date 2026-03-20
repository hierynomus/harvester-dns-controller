//! Kubernetes event publishing for DNS operations.
//!
//! Emits events on the source objects (VMNC, LoadBalancer) so they appear
//! in `kubectl describe` output and can be collected by observability tools.

use k8s_openapi::api::core::v1::{Event, EventSource, ObjectReference};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::MicroTime;
use k8s_openapi::chrono::Utc;
use kube::api::{Api, PostParams};
use kube::{Client, Resource, ResourceExt};
use tracing::{debug, warn};

use crate::registry::DnsAction;

/// Component name used in event source.
const COMPONENT: &str = "harvester-dns-controller";

/// Event reasons for DNS operations.
#[derive(Debug, Clone, Copy)]
pub enum DnsEventReason {
    /// DNS record was created.
    DnsRecordCreated,
    /// DNS record was updated (IP changed).
    DnsRecordUpdated,
    /// DNS record was deleted.
    DnsRecordDeleted,
    /// DNS record creation/update failed.
    DnsRecordFailed,
}

impl DnsEventReason {
    fn as_str(&self) -> &'static str {
        match self {
            DnsEventReason::DnsRecordCreated => "DnsRecordCreated",
            DnsEventReason::DnsRecordUpdated => "DnsRecordUpdated",
            DnsEventReason::DnsRecordDeleted => "DnsRecordDeleted",
            DnsEventReason::DnsRecordFailed => "DnsRecordFailed",
        }
    }

    fn event_type(&self) -> &'static str {
        match self {
            DnsEventReason::DnsRecordFailed => "Warning",
            _ => "Normal",
        }
    }
}

/// Publishes Kubernetes events for DNS operations.
#[derive(Clone)]
pub struct EventRecorder {
    client: Client,
}

impl EventRecorder {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Publish an event on a Kubernetes resource.
    ///
    /// Events are best-effort — failures are logged but not propagated.
    pub async fn publish<K>(
        &self,
        resource: &K,
        reason: DnsEventReason,
        message: String,
    ) where
        K: Resource<Scope = kube::core::NamespaceResourceScope> + ResourceExt,
        K::DynamicType: Default,
    {
        let namespace = resource.namespace().unwrap_or_default();
        let name = resource.name_any();
        let uid = resource.uid().unwrap_or_default();

        let dt = K::DynamicType::default();
        let obj_ref = ObjectReference {
            api_version: Some(K::api_version(&dt).to_string()),
            kind: Some(K::kind(&dt).to_string()),
            name: Some(name.clone()),
            namespace: Some(namespace.clone()),
            uid: Some(uid),
            ..Default::default()
        };

        let now = Utc::now();
        let event_name = format!(
            "{}.{:x}",
            name,
            now.timestamp_nanos_opt().unwrap_or(0) as u64
        );

        let event = Event {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(event_name),
                namespace: Some(namespace.clone()),
                ..Default::default()
            },
            involved_object: obj_ref,
            reason: Some(reason.as_str().to_string()),
            message: Some(message.clone()),
            type_: Some(reason.event_type().to_string()),
            event_time: Some(MicroTime(now)),
            reporting_component: Some(COMPONENT.to_string()),
            reporting_instance: Some(
                std::env::var("POD_NAME").unwrap_or_else(|_| COMPONENT.to_string()),
            ),
            source: Some(EventSource {
                component: Some(COMPONENT.to_string()),
                ..Default::default()
            }),
            first_timestamp: Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(now)),
            last_timestamp: Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(now)),
            count: Some(1),
            action: Some(reason.as_str().to_string()),
            ..Default::default()
        };

        let api: Api<Event> = Api::namespaced(self.client.clone(), &namespace);

        match api.create(&PostParams::default(), &event).await {
            Ok(_) => {
                debug!(
                    resource = %name,
                    namespace = %namespace,
                    reason = %reason.as_str(),
                    "Published DNS event"
                );
            }
            Err(e) => {
                // Events are best-effort — don't fail reconciliation
                warn!(
                    resource = %name,
                    namespace = %namespace,
                    reason = %reason.as_str(),
                    error = %e,
                    "Failed to publish DNS event"
                );
            }
        }
    }

    /// Helper: emit a "record created" event.
    pub async fn record_created<K>(&self, resource: &K, fqdn: &str, ip: &str)
    where
        K: Resource<Scope = kube::core::NamespaceResourceScope> + ResourceExt,
        K::DynamicType: Default,
    {
        self.publish(
            resource,
            DnsEventReason::DnsRecordCreated,
            format!("Created DNS record {} -> {}", fqdn, ip),
        )
        .await;
    }

    /// Helper: emit a "record updated" event.
    pub async fn record_updated<K>(&self, resource: &K, fqdn: &str, ip: &str)
    where
        K: Resource<Scope = kube::core::NamespaceResourceScope> + ResourceExt,
        K::DynamicType: Default,
    {
        self.publish(
            resource,
            DnsEventReason::DnsRecordUpdated,
            format!("Updated DNS record {} -> {}", fqdn, ip),
        )
        .await;
    }

    /// Helper: emit a "record deleted" event.
    pub async fn record_deleted<K>(&self, resource: &K, fqdn: &str)
    where
        K: Resource<Scope = kube::core::NamespaceResourceScope> + ResourceExt,
        K::DynamicType: Default,
    {
        self.publish(
            resource,
            DnsEventReason::DnsRecordDeleted,
            format!("Deleted DNS record {}", fqdn),
        )
        .await;
    }

    /// Helper: emit a "record failed" event.
    pub async fn record_failed<K>(&self, resource: &K, fqdn: &str, error: &str)
    where
        K: Resource<Scope = kube::core::NamespaceResourceScope> + ResourceExt,
        K::DynamicType: Default,
    {
        self.publish(
            resource,
            DnsEventReason::DnsRecordFailed,
            format!("Failed to manage DNS record {}: {}", fqdn, error),
        )
        .await;
    }

    /// Emit the appropriate event based on the DNS action taken.
    pub async fn emit_for_action<K>(
        &self,
        resource: &K,
        action: &DnsAction,
        fqdn: &str,
        ip: Option<&str>,
    ) where
        K: Resource<Scope = kube::core::NamespaceResourceScope> + ResourceExt,
        K::DynamicType: Default,
    {
        match action {
            DnsAction::Created => {
                if let Some(ip) = ip {
                    self.record_created(resource, fqdn, ip).await;
                }
            }
            DnsAction::Updated => {
                if let Some(ip) = ip {
                    self.record_updated(resource, fqdn, ip).await;
                }
            }
            DnsAction::Deleted => {
                self.record_deleted(resource, fqdn).await;
            }
            DnsAction::Unchanged | DnsAction::None => {
                // No event needed for unchanged records
            }
        }
    }
}
