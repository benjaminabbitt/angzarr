//! K8s label-based service discovery.
//!
//! Discovers aggregate and projector services by watching K8s Service
//! resources with appropriate labels. Service mesh handles L7 gRPC load
//! balancing—we just connect to Service DNS names.
//!
//! # Label Scheme
//!
//! ```yaml
//! # Aggregate coordinator
//! labels:
//!   app.kubernetes.io/component: aggregate
//!   angzarr.io/domain: cart
//!
//! # Projector coordinator
//! labels:
//!   app.kubernetes.io/component: projector
//!   angzarr.io/domain: cart
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use futures::TryStreamExt;
use k8s_openapi::api::core::v1::Service;
use kube::{
    api::{Api, ListParams},
    runtime::watcher::{self, Event},
    Client,
};
use tokio::sync::RwLock;
use tonic::transport::Channel;
use tracing::{debug, error, info};

use crate::config::{NAMESPACE_ENV_VAR, POD_NAMESPACE_ENV_VAR};
use crate::proto::aggregate_coordinator_service_client::AggregateCoordinatorServiceClient;
use crate::proto::event_query_service_client::EventQueryServiceClient;
use crate::proto::projector_coordinator_service_client::ProjectorCoordinatorServiceClient;

use super::static_discovery::StaticServiceDiscovery;

/// Label for component type.
const COMPONENT_LABEL: &str = "app.kubernetes.io/component";

/// Label for domain (aggregate and projector).
const DOMAIN_LABEL: &str = "angzarr.io/domain";

/// Component values.
const COMPONENT_AGGREGATE: &str = "aggregate";
const COMPONENT_PROJECTOR: &str = "projector";

/// Default gRPC port.
const DEFAULT_GRPC_PORT: u16 = 50051;

/// Error types for service discovery.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("Service not found for domain: {0}")]
    DomainNotFound(String),

    #[error("No services found for component: {0}")]
    NoServicesFound(String),

    #[error("Connection failed to {service} at {address}: {message}")]
    ConnectionFailed {
        service: String,
        address: String,
        message: String,
    },

    #[error("Kubernetes API error: {0}")]
    KubeError(#[from] kube::Error),
}

/// A discovered K8s service.
#[derive(Debug, Clone)]
pub struct DiscoveredService {
    /// Service name.
    pub name: String,
    /// Full DNS address (service.namespace.svc.cluster.local).
    pub service_address: String,
    /// gRPC port.
    pub port: u16,
    /// Domain this service handles (angzarr.io/domain).
    pub domain: Option<String>,
}

impl DiscoveredService {
    /// Get the gRPC endpoint URL or path.
    ///
    /// For TCP endpoints, returns `http://address:port`.
    /// For UDS endpoints (starting with `/`), returns the path as-is.
    pub fn grpc_url(&self) -> String {
        if self.service_address.starts_with('/') {
            // UDS path - return as-is
            self.service_address.clone()
        } else {
            // TCP address
            format!("http://{}:{}", self.service_address, self.port)
        }
    }
}

/// K8s label-based service discovery.
///
/// Mesh handles L7 load balancing—we just connect to Service names.
/// Delegates storage and client caching to `StaticServiceDiscovery`.
pub struct K8sServiceDiscovery {
    client: Option<Client>,
    namespace: String,
    /// Aggregates cache for K8s watcher updates.
    aggregates: Arc<RwLock<HashMap<String, DiscoveredService>>>,
    /// Projectors cache for K8s watcher updates.
    projectors: Arc<RwLock<HashMap<String, DiscoveredService>>>,
    /// Inner static discovery for storage and client caching.
    inner: StaticServiceDiscovery,
}

impl K8sServiceDiscovery {
    /// Create a new service discovery instance.
    pub async fn new(namespace: impl Into<String>) -> Result<Self, DiscoveryError> {
        let client = Client::try_default().await?;
        let namespace = namespace.into();

        info!(namespace = %namespace, "Service discovery initialized");

        Ok(Self {
            client: Some(client),
            namespace: namespace.clone(),
            aggregates: Arc::new(RwLock::new(HashMap::new())),
            projectors: Arc::new(RwLock::new(HashMap::new())),
            inner: StaticServiceDiscovery::new(&namespace),
        })
    }

    /// Create a static instance without K8s client.
    ///
    /// **Deprecated**: Use `StaticServiceDiscovery::new()` directly instead.
    /// This method exists for backwards compatibility.
    pub fn new_static() -> Self {
        Self {
            client: None,
            namespace: "static".to_string(),
            aggregates: Arc::new(RwLock::new(HashMap::new())),
            projectors: Arc::new(RwLock::new(HashMap::new())),
            inner: StaticServiceDiscovery::new("static"),
        }
    }

    /// Create from environment variables.
    ///
    /// Reads namespace from NAMESPACE_ENV_VAR or POD_NAMESPACE_ENV_VAR env vars.
    pub async fn from_env() -> Result<Self, DiscoveryError> {
        let namespace = std::env::var(NAMESPACE_ENV_VAR)
            .or_else(|_| std::env::var(POD_NAMESPACE_ENV_VAR))
            .unwrap_or_else(|_| "default".to_string());

        Self::new(namespace).await
    }

    fn start_watching_component(
        &self,
        component: &'static str,
        cache: Arc<RwLock<HashMap<String, DiscoveredService>>>,
    ) {
        let client = match &self.client {
            Some(c) => c.clone(),
            None => return,
        };
        let namespace = self.namespace.clone();

        tokio::spawn(async move {
            let services: Api<Service> = Api::namespaced(client, &namespace);

            let watcher = watcher::watcher(
                services,
                watcher::Config::default().labels(&format!("{}={}", COMPONENT_LABEL, component)),
            );

            info!(component = component, "Starting service watcher");

            if let Err(e) = watcher
                .try_for_each(|event| {
                    let cache = cache.clone();
                    async move {
                        Self::handle_event(component, &cache, event).await;
                        Ok(())
                    }
                })
                .await
            {
                error!(component = component, error = %e, "Service watcher error");
            }
        });
    }

    async fn handle_event(
        component: &str,
        cache: &RwLock<HashMap<String, DiscoveredService>>,
        event: Event<Service>,
    ) {
        match event {
            Event::Apply(svc) | Event::InitApply(svc) => {
                if let Some(discovered) = Self::extract_service_static(&svc) {
                    debug!(
                        component = component,
                        service = %discovered.name,
                        domain = ?discovered.domain,
                        "Service discovered/updated"
                    );
                    cache
                        .write()
                        .await
                        .insert(discovered.name.clone(), discovered);
                }
            }
            Event::Delete(svc) => {
                if let Some(name) = svc.metadata.name {
                    debug!(component = component, service = %name, "Service deleted");
                    cache.write().await.remove(&name);
                }
            }
            Event::Init => {
                debug!(component = component, "Watcher initialized");
            }
            Event::InitDone => {
                debug!(component = component, "Watcher init done");
            }
        }
    }

    fn extract_service(&self, svc: &Service) -> Option<DiscoveredService> {
        Self::extract_service_with_namespace(svc, &self.namespace)
    }

    fn extract_service_static(svc: &Service) -> Option<DiscoveredService> {
        let namespace = svc.metadata.namespace.as_deref().unwrap_or("default");
        Self::extract_service_with_namespace(svc, namespace)
    }

    fn extract_service_with_namespace(svc: &Service, namespace: &str) -> Option<DiscoveredService> {
        let name = svc.metadata.name.as_ref()?;
        let labels = svc.metadata.labels.as_ref();

        let domain = labels.and_then(|l| l.get(DOMAIN_LABEL)).cloned();

        // Find grpc port
        let port = svc
            .spec
            .as_ref()
            .and_then(|s| s.ports.as_ref())
            .and_then(|ports| {
                ports
                    .iter()
                    .find(|p| p.name.as_deref() == Some("grpc"))
                    .or_else(|| ports.first())
            })
            .and_then(|p| u16::try_from(p.port).ok())
            .unwrap_or(DEFAULT_GRPC_PORT);

        let service_address = format!("{}.{}.svc.cluster.local", name, namespace);

        info!(
            service = %name,
            address = %service_address,
            port = port,
            domain = ?domain,
            "Extracted service"
        );

        Some(DiscoveredService {
            name: name.clone(),
            service_address,
            port,
            domain,
        })
    }

    /// Register a discovered service with inner for client caching.
    async fn sync_to_inner(&self, component: &str, service: &DiscoveredService) {
        if let Some(domain) = &service.domain {
            if component == COMPONENT_AGGREGATE {
                self.inner
                    .register_aggregate(domain, &service.service_address, service.port)
                    .await;
            } else if component == COMPONENT_PROJECTOR {
                self.inner
                    .register_projector(
                        &service.name,
                        domain,
                        &service.service_address,
                        service.port,
                    )
                    .await;
            }
        }
    }
}

use super::ServiceDiscovery;

#[async_trait::async_trait]
impl ServiceDiscovery for K8sServiceDiscovery {
    async fn register_aggregate(&self, domain: &str, address: &str, port: u16) {
        // Store in local cache for K8s compatibility
        let service = DiscoveredService {
            name: format!("{}-aggregate", domain),
            service_address: address.to_string(),
            port,
            domain: Some(domain.to_string()),
        };
        self.aggregates
            .write()
            .await
            .insert(service.name.clone(), service);

        // Delegate to inner for client caching
        self.inner.register_aggregate(domain, address, port).await;
    }

    async fn register_projector(&self, name: &str, domain: &str, address: &str, port: u16) {
        // Store in local cache for K8s compatibility
        let service = DiscoveredService {
            name: name.to_string(),
            service_address: address.to_string(),
            port,
            domain: Some(domain.to_string()),
        };
        self.projectors
            .write()
            .await
            .insert(service.name.clone(), service);

        // Delegate to inner for client caching
        self.inner
            .register_projector(name, domain, address, port)
            .await;
    }

    async fn get_aggregate(
        &self,
        domain: &str,
    ) -> Result<AggregateCoordinatorServiceClient<Channel>, DiscoveryError> {
        // Sync any unsynced services from local cache to inner
        let aggregates = self.aggregates.read().await;
        for service in aggregates.values() {
            if let Some(d) = &service.domain {
                // This is idempotent - inner will skip if already registered
                self.inner
                    .register_aggregate(d, &service.service_address, service.port)
                    .await;
            }
        }
        drop(aggregates);

        // Delegate to inner
        self.inner.get_aggregate(domain).await
    }

    async fn get_event_query(
        &self,
        domain: &str,
    ) -> Result<EventQueryServiceClient<Channel>, DiscoveryError> {
        // Sync any unsynced services from local cache to inner
        let aggregates = self.aggregates.read().await;
        for service in aggregates.values() {
            if let Some(d) = &service.domain {
                self.inner
                    .register_aggregate(d, &service.service_address, service.port)
                    .await;
            }
        }
        drop(aggregates);

        // Delegate to inner
        self.inner.get_event_query(domain).await
    }

    async fn get_all_projectors(
        &self,
    ) -> Result<Vec<ProjectorCoordinatorServiceClient<Channel>>, DiscoveryError> {
        // Sync any unsynced services from local cache to inner
        let projectors = self.projectors.read().await;
        for service in projectors.values() {
            if let Some(d) = &service.domain {
                self.inner
                    .register_projector(&service.name, d, &service.service_address, service.port)
                    .await;
            }
        }
        drop(projectors);

        // Delegate to inner
        self.inner.get_all_projectors().await
    }

    async fn get_projector_by_name(
        &self,
        name: &str,
    ) -> Result<ProjectorCoordinatorServiceClient<Channel>, DiscoveryError> {
        // Sync any unsynced services from local cache to inner
        let projectors = self.projectors.read().await;
        for service in projectors.values() {
            if let Some(d) = &service.domain {
                self.inner
                    .register_projector(&service.name, d, &service.service_address, service.port)
                    .await;
            }
        }
        drop(projectors);

        // Delegate to inner
        self.inner.get_projector_by_name(name).await
    }

    async fn aggregate_domains(&self) -> Vec<String> {
        // Use local cache - it has the authoritative list from K8s
        self.aggregates
            .read()
            .await
            .values()
            .filter_map(|s| s.domain.clone())
            .collect()
    }

    async fn has_aggregates(&self) -> bool {
        !self.aggregates.read().await.is_empty()
    }

    async fn has_projectors(&self) -> bool {
        !self.projectors.read().await.is_empty()
    }

    async fn initial_sync(&self) -> Result<(), DiscoveryError> {
        let client = match &self.client {
            Some(c) => c.clone(),
            None => return Ok(()), // Static mode - no K8s sync
        };

        info!("Performing initial service sync");

        let services: Api<Service> = Api::namespaced(client, &self.namespace);

        // Sync aggregates
        let aggregate_list = services
            .list(
                &ListParams::default()
                    .labels(&format!("{}={}", COMPONENT_LABEL, COMPONENT_AGGREGATE)),
            )
            .await?;
        for svc in aggregate_list {
            if let Some(discovered) = self.extract_service(&svc) {
                self.aggregates
                    .write()
                    .await
                    .insert(discovered.name.clone(), discovered.clone());
                // Also register with inner
                self.sync_to_inner(COMPONENT_AGGREGATE, &discovered).await;
            }
        }

        // Sync projectors
        let projector_list = services
            .list(
                &ListParams::default()
                    .labels(&format!("{}={}", COMPONENT_LABEL, COMPONENT_PROJECTOR)),
            )
            .await?;
        for svc in projector_list {
            if let Some(discovered) = self.extract_service(&svc) {
                self.projectors
                    .write()
                    .await
                    .insert(discovered.name.clone(), discovered.clone());
                // Also register with inner
                self.sync_to_inner(COMPONENT_PROJECTOR, &discovered).await;
            }
        }

        let aggregates = self.aggregates.read().await;
        let projectors = self.projectors.read().await;

        info!(
            aggregates = aggregates.len(),
            projectors = projectors.len(),
            "Initial sync complete"
        );

        Ok(())
    }

    fn start_watching(&self) {
        if self.client.is_none() {
            return; // Static mode - no K8s watching
        }
        self.start_watching_component(COMPONENT_AGGREGATE, self.aggregates.clone());
        self.start_watching_component(COMPONENT_PROJECTOR, self.projectors.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{ServicePort, ServiceSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

    fn make_test_service(name: &str, component: &str, domain: Option<&str>, port: i32) -> Service {
        let mut labels = BTreeMap::new();
        labels.insert(COMPONENT_LABEL.to_string(), component.to_string());
        if let Some(d) = domain {
            labels.insert(DOMAIN_LABEL.to_string(), d.to_string());
        }

        Service {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some("test-ns".to_string()),
                labels: Some(labels),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                ports: Some(vec![ServicePort {
                    name: Some("grpc".to_string()),
                    port,
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            status: None,
        }
    }

    #[test]
    fn test_extract_aggregate_service() {
        let svc = make_test_service("cart-agg", COMPONENT_AGGREGATE, Some("cart"), 50051);
        let discovered =
            K8sServiceDiscovery::extract_service_with_namespace(&svc, "test-ns").unwrap();

        assert_eq!(discovered.name, "cart-agg");
        assert_eq!(
            discovered.service_address,
            "cart-agg.test-ns.svc.cluster.local"
        );
        assert_eq!(discovered.port, 50051);
        assert_eq!(discovered.domain, Some("cart".to_string()));
    }

    #[test]
    fn test_extract_projector_service() {
        let svc = make_test_service("cart-proj", COMPONENT_PROJECTOR, Some("cart"), 50052);
        let discovered =
            K8sServiceDiscovery::extract_service_with_namespace(&svc, "test-ns").unwrap();

        assert_eq!(discovered.name, "cart-proj");
        assert_eq!(discovered.domain, Some("cart".to_string()));
    }

    #[test]
    fn test_grpc_url() {
        let service = DiscoveredService {
            name: "test-svc".to_string(),
            service_address: "test-svc.ns.svc.cluster.local".to_string(),
            port: 50051,
            domain: None,
        };

        assert_eq!(
            service.grpc_url(),
            "http://test-svc.ns.svc.cluster.local:50051"
        );
    }

    #[test]
    fn test_extract_service_without_grpc_port_uses_default() {
        let svc = Service {
            metadata: ObjectMeta {
                name: Some("test-svc".to_string()),
                namespace: Some("test-ns".to_string()),
                labels: Some({
                    let mut l = BTreeMap::new();
                    l.insert(COMPONENT_LABEL.to_string(), COMPONENT_AGGREGATE.to_string());
                    l
                }),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                ports: None,
                ..Default::default()
            }),
            status: None,
        };

        let discovered =
            K8sServiceDiscovery::extract_service_with_namespace(&svc, "test-ns").unwrap();
        assert_eq!(discovered.port, DEFAULT_GRPC_PORT);
    }
}
