//! K8s label-based service discovery.
//!
//! Discovers aggregate and projector coordinator services by watching
//! K8s Service resources with appropriate labels. Service mesh handles L7
//! gRPC load balancing—we just connect to Service DNS names.
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
use tracing::{debug, error, info, warn};

use crate::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use crate::proto::event_query_client::EventQueryClient;
use crate::proto::projector_coordinator_client::ProjectorCoordinatorClient;

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

/// Get or create a cached gRPC client connection.
///
/// Checks the cache first, creates a new connection if not found,
/// and caches the result for future use.
///
/// Supports both TCP (http://host:port) and UDS (/path/to/socket.sock) addresses.
async fn get_or_create_client<C, F>(
    cache: &RwLock<HashMap<String, C>>,
    service: &DiscoveredService,
    client_type: &str,
    connect_fn: F,
) -> Result<C, DiscoveryError>
where
    C: Clone,
    F: FnOnce(Channel) -> C,
{
    let url = service.grpc_url();

    // Check cache
    {
        let clients = cache.read().await;
        if let Some(client) = clients.get(&url) {
            debug!(service = %service.name, client_type = %client_type, "Using cached client");
            return Ok(client.clone());
        }
    }

    // Create new connection - handle both TCP and UDS
    info!(service = %service.name, url = %url, client_type = %client_type, "Creating client");
    let channel = crate::transport::connect_to_address(&url)
        .await
        .map_err(|e| DiscoveryError::ConnectionFailed {
            service: service.name.clone(),
            address: url.clone(),
            message: e.to_string(),
        })?;

    let client = connect_fn(channel);

    // Cache
    cache.write().await.insert(url, client.clone());

    Ok(client)
}

/// Create an empty RwLock-wrapped HashMap.
fn empty_cache<K, V>() -> Arc<RwLock<HashMap<K, V>>> {
    Arc::new(RwLock::new(HashMap::new()))
}

/// K8s label-based service discovery.
///
/// Mesh handles L7 load balancing—we just connect to Service names.
pub struct K8sServiceDiscovery {
    client: Option<Client>,
    namespace: String,
    aggregates: Arc<RwLock<HashMap<String, DiscoveredService>>>,
    projectors: Arc<RwLock<HashMap<String, DiscoveredService>>>,
    // Connection caches
    aggregate_clients: Arc<RwLock<HashMap<String, AggregateCoordinatorClient<Channel>>>>,
    event_query_clients: Arc<RwLock<HashMap<String, EventQueryClient<Channel>>>>,
    projector_clients: Arc<RwLock<HashMap<String, ProjectorCoordinatorClient<Channel>>>>,
}

impl K8sServiceDiscovery {
    /// Create a new service discovery instance.
    pub async fn new(namespace: impl Into<String>) -> Result<Self, DiscoveryError> {
        let client = Client::try_default().await?;
        let namespace = namespace.into();

        info!(namespace = %namespace, "Service discovery initialized");

        Ok(Self {
            client: Some(client),
            namespace,
            aggregates: empty_cache(),
            projectors: empty_cache(),
            aggregate_clients: empty_cache(),
            event_query_clients: empty_cache(),
            projector_clients: empty_cache(),
        })
    }

    /// Create a static instance without K8s client.
    ///
    /// Use `register_aggregate` to add services manually.
    /// Useful for embedded mode and testing.
    pub fn new_static() -> Self {
        Self {
            client: None,
            namespace: "static".to_string(),
            aggregates: empty_cache(),
            projectors: empty_cache(),
            aggregate_clients: empty_cache(),
            event_query_clients: empty_cache(),
            projector_clients: empty_cache(),
        }
    }

    /// Create from environment variables.
    ///
    /// Reads namespace from NAMESPACE or POD_NAMESPACE env vars.
    pub async fn from_env() -> Result<Self, DiscoveryError> {
        let namespace = std::env::var("NAMESPACE")
            .or_else(|_| std::env::var("POD_NAMESPACE"))
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

    async fn get_or_create_aggregate_client(
        &self,
        service: &DiscoveredService,
    ) -> Result<AggregateCoordinatorClient<Channel>, DiscoveryError> {
        get_or_create_client(
            &self.aggregate_clients,
            service,
            "aggregate",
            AggregateCoordinatorClient::new,
        )
        .await
    }

    async fn get_or_create_event_query_client(
        &self,
        service: &DiscoveredService,
    ) -> Result<EventQueryClient<Channel>, DiscoveryError> {
        get_or_create_client(
            &self.event_query_clients,
            service,
            "event_query",
            EventQueryClient::new,
        )
        .await
    }

    async fn get_or_create_projector_client(
        &self,
        service: &DiscoveredService,
    ) -> Result<ProjectorCoordinatorClient<Channel>, DiscoveryError> {
        get_or_create_client(
            &self.projector_clients,
            service,
            "projector",
            ProjectorCoordinatorClient::new,
        )
        .await
    }
}

#[async_trait::async_trait]
impl super::ServiceDiscovery for K8sServiceDiscovery {
    async fn register_aggregate(&self, domain: &str, address: &str, port: u16) {
        let service = DiscoveredService {
            name: format!("{}-aggregate", domain),
            service_address: address.to_string(),
            port,
            domain: Some(domain.to_string()),
        };
        info!(
            domain = %domain,
            address = %address,
            port = port,
            "Registered static aggregate"
        );
        self.aggregates
            .write()
            .await
            .insert(service.name.clone(), service);
    }

    async fn register_projector(&self, name: &str, domain: &str, address: &str, port: u16) {
        let service = DiscoveredService {
            name: name.to_string(),
            service_address: address.to_string(),
            port,
            domain: Some(domain.to_string()),
        };
        info!(
            name = %name,
            domain = %domain,
            address = %address,
            port = port,
            "Registered static projector"
        );
        self.projectors
            .write()
            .await
            .insert(service.name.clone(), service);
    }

    async fn get_aggregate(
        &self,
        domain: &str,
    ) -> Result<AggregateCoordinatorClient<Channel>, DiscoveryError> {
        let aggregates = self.aggregates.read().await;

        // Find service matching domain, or wildcard
        let service = aggregates
            .values()
            .find(|s| s.domain.as_deref() == Some(domain))
            .or_else(|| {
                aggregates
                    .values()
                    .find(|s| s.domain.as_deref() == Some("*"))
            })
            .ok_or_else(|| DiscoveryError::DomainNotFound(domain.to_string()))?
            .clone();

        drop(aggregates);

        self.get_or_create_aggregate_client(&service).await
    }

    async fn get_event_query(
        &self,
        domain: &str,
    ) -> Result<EventQueryClient<Channel>, DiscoveryError> {
        let aggregates = self.aggregates.read().await;

        // Find service matching domain, or wildcard
        let service = aggregates
            .values()
            .find(|s| s.domain.as_deref() == Some(domain))
            .or_else(|| {
                aggregates
                    .values()
                    .find(|s| s.domain.as_deref() == Some("*"))
            })
            .cloned();

        drop(aggregates);

        if let Some(service) = service {
            return self.get_or_create_event_query_client(&service).await;
        }

        // Fallback to EVENT_QUERY_ADDRESS env var
        if let Ok(addr) = std::env::var("EVENT_QUERY_ADDRESS") {
            // Parse address - may be "host:port" or "http://host:port"
            let (host, port) = if addr.starts_with("http://") || addr.starts_with("https://") {
                // Already a URL, extract host:port
                let without_scheme = addr
                    .trim_start_matches("http://")
                    .trim_start_matches("https://");
                if let Some((h, p)) = without_scheme.rsplit_once(':') {
                    (h.to_string(), p.parse().unwrap_or(80))
                } else {
                    (without_scheme.to_string(), 80)
                }
            } else if let Some((h, p)) = addr.rsplit_once(':') {
                // host:port format
                (h.to_string(), p.parse().unwrap_or(80))
            } else {
                // Just host, default port
                (addr, 80)
            };

            let service = DiscoveredService {
                name: format!("{}-event-query-fallback", domain),
                service_address: host,
                port,
                domain: Some(domain.to_string()),
            };
            return self.get_or_create_event_query_client(&service).await;
        }

        Err(DiscoveryError::DomainNotFound(domain.to_string()))
    }

    async fn get_all_projectors(
        &self,
    ) -> Result<Vec<ProjectorCoordinatorClient<Channel>>, DiscoveryError> {
        let projectors = self.projectors.read().await;

        if projectors.is_empty() {
            return Ok(vec![]);
        }

        let services: Vec<_> = projectors.values().cloned().collect();
        drop(projectors);

        let mut clients = Vec::with_capacity(services.len());
        for service in services {
            match self.get_or_create_projector_client(&service).await {
                Ok(client) => clients.push(client),
                Err(e) => {
                    warn!(service = %service.name, error = %e, "Failed to get projector client")
                }
            }
        }

        Ok(clients)
    }

    async fn aggregate_domains(&self) -> Vec<String> {
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
            None => return Ok(()), // Test mode - no K8s sync
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
                    .insert(discovered.name.clone(), discovered);
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
                    .insert(discovered.name.clone(), discovered);
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
            return; // Test mode - no K8s watching
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
