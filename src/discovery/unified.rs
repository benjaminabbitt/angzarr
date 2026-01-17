//! Unified service discovery using K8s labels.
//!
//! Discovers aggregate, projector, and saga coordinator services by watching
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
//!
//! # Saga coordinator
//! labels:
//!   app.kubernetes.io/component: saga
//!   angzarr.io/source-domain: order
//!   angzarr.io/domain: fulfillment
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
use crate::proto::projector_coordinator_client::ProjectorCoordinatorClient;
use crate::proto::saga_coordinator_client::SagaCoordinatorClient;

/// Label for component type.
const COMPONENT_LABEL: &str = "app.kubernetes.io/component";

/// Label for domain (aggregate and projector).
const DOMAIN_LABEL: &str = "angzarr.io/domain";

/// Label for source domain (saga - events it listens to).
const SOURCE_DOMAIN_LABEL: &str = "angzarr.io/source-domain";

/// Component values.
const COMPONENT_AGGREGATE: &str = "aggregate";
const COMPONENT_PROJECTOR: &str = "projector";
const COMPONENT_SAGA: &str = "saga";

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
    /// Source domain for sagas (angzarr.io/source-domain).
    pub source_domain: Option<String>,
}

impl DiscoveredService {
    /// Get the gRPC endpoint URL.
    pub fn grpc_url(&self) -> String {
        format!("http://{}:{}", self.service_address, self.port)
    }
}

/// Unified service discovery using K8s labels.
///
/// Mesh handles L7 load balancing—we just connect to Service names.
pub struct ServiceDiscovery {
    client: Client,
    namespace: String,
    aggregates: Arc<RwLock<HashMap<String, DiscoveredService>>>,
    projectors: Arc<RwLock<HashMap<String, DiscoveredService>>>,
    sagas: Arc<RwLock<HashMap<String, DiscoveredService>>>,
    // Connection caches
    aggregate_clients: Arc<RwLock<HashMap<String, AggregateCoordinatorClient<Channel>>>>,
    projector_clients: Arc<RwLock<HashMap<String, ProjectorCoordinatorClient<Channel>>>>,
    saga_clients: Arc<RwLock<HashMap<String, SagaCoordinatorClient<Channel>>>>,
}

impl ServiceDiscovery {
    /// Create a new service discovery instance.
    pub async fn new(namespace: impl Into<String>) -> Result<Self, DiscoveryError> {
        let client = Client::try_default().await?;
        let namespace = namespace.into();

        info!(namespace = %namespace, "Service discovery initialized");

        Ok(Self {
            client,
            namespace,
            aggregates: Arc::new(RwLock::new(HashMap::new())),
            projectors: Arc::new(RwLock::new(HashMap::new())),
            sagas: Arc::new(RwLock::new(HashMap::new())),
            aggregate_clients: Arc::new(RwLock::new(HashMap::new())),
            projector_clients: Arc::new(RwLock::new(HashMap::new())),
            saga_clients: Arc::new(RwLock::new(HashMap::new())),
        })
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

    /// Perform initial sync of all services.
    pub async fn initial_sync(&self) -> Result<(), DiscoveryError> {
        info!("Performing initial service sync");

        let services: Api<Service> = Api::namespaced(self.client.clone(), &self.namespace);

        // Sync aggregates
        let aggregate_list = services
            .list(&ListParams::default().labels(&format!("{}={}", COMPONENT_LABEL, COMPONENT_AGGREGATE)))
            .await?;
        for svc in aggregate_list {
            if let Some(discovered) = self.extract_service(&svc) {
                self.aggregates.write().await.insert(discovered.name.clone(), discovered);
            }
        }

        // Sync projectors
        let projector_list = services
            .list(&ListParams::default().labels(&format!("{}={}", COMPONENT_LABEL, COMPONENT_PROJECTOR)))
            .await?;
        for svc in projector_list {
            if let Some(discovered) = self.extract_service(&svc) {
                self.projectors.write().await.insert(discovered.name.clone(), discovered);
            }
        }

        // Sync sagas
        let saga_list = services
            .list(&ListParams::default().labels(&format!("{}={}", COMPONENT_LABEL, COMPONENT_SAGA)))
            .await?;
        for svc in saga_list {
            if let Some(discovered) = self.extract_service(&svc) {
                self.sagas.write().await.insert(discovered.name.clone(), discovered);
            }
        }

        let aggregates = self.aggregates.read().await;
        let projectors = self.projectors.read().await;
        let sagas = self.sagas.read().await;

        info!(
            aggregates = aggregates.len(),
            projectors = projectors.len(),
            sagas = sagas.len(),
            "Initial sync complete"
        );

        Ok(())
    }

    /// Start watching for service changes in the background.
    pub fn start_watching(&self) {
        self.start_watching_component(COMPONENT_AGGREGATE, self.aggregates.clone());
        self.start_watching_component(COMPONENT_PROJECTOR, self.projectors.clone());
        self.start_watching_component(COMPONENT_SAGA, self.sagas.clone());
    }

    fn start_watching_component(
        &self,
        component: &'static str,
        cache: Arc<RwLock<HashMap<String, DiscoveredService>>>,
    ) {
        let client = self.client.clone();
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
                        source_domain = ?discovered.source_domain,
                        "Service discovered/updated"
                    );
                    cache.write().await.insert(discovered.name.clone(), discovered);
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
        let source_domain = labels.and_then(|l| l.get(SOURCE_DOMAIN_LABEL)).cloned();

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
            source_domain = ?source_domain,
            "Extracted service"
        );

        Some(DiscoveredService {
            name: name.clone(),
            service_address,
            port,
            domain,
            source_domain,
        })
    }

    /// Get aggregate coordinator client by domain.
    pub async fn get_aggregate(
        &self,
        domain: &str,
    ) -> Result<AggregateCoordinatorClient<Channel>, DiscoveryError> {
        let aggregates = self.aggregates.read().await;

        // Find service matching domain, or wildcard
        let service = aggregates
            .values()
            .find(|s| s.domain.as_deref() == Some(domain))
            .or_else(|| aggregates.values().find(|s| s.domain.as_deref() == Some("*")))
            .ok_or_else(|| DiscoveryError::DomainNotFound(domain.to_string()))?
            .clone();

        drop(aggregates);

        self.get_or_create_aggregate_client(&service).await
    }

    async fn get_or_create_aggregate_client(
        &self,
        service: &DiscoveredService,
    ) -> Result<AggregateCoordinatorClient<Channel>, DiscoveryError> {
        let url = service.grpc_url();

        // Check cache
        {
            let clients = self.aggregate_clients.read().await;
            if let Some(client) = clients.get(&url) {
                debug!(service = %service.name, "Using cached aggregate client");
                return Ok(client.clone());
            }
        }

        // Create new connection
        info!(service = %service.name, url = %url, "Creating aggregate client");
        let client = AggregateCoordinatorClient::connect(url.clone())
            .await
            .map_err(|e| DiscoveryError::ConnectionFailed {
                service: service.name.clone(),
                address: url.clone(),
                message: e.to_string(),
            })?;

        // Cache
        self.aggregate_clients.write().await.insert(url, client.clone());

        Ok(client)
    }

    /// Get all projector coordinator clients.
    ///
    /// Returns one client per projector service. Mesh handles pod-level LB.
    pub async fn get_all_projectors(
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
                Err(e) => warn!(service = %service.name, error = %e, "Failed to get projector client"),
            }
        }

        Ok(clients)
    }

    async fn get_or_create_projector_client(
        &self,
        service: &DiscoveredService,
    ) -> Result<ProjectorCoordinatorClient<Channel>, DiscoveryError> {
        let url = service.grpc_url();

        // Check cache
        {
            let clients = self.projector_clients.read().await;
            if let Some(client) = clients.get(&url) {
                debug!(service = %service.name, "Using cached projector client");
                return Ok(client.clone());
            }
        }

        // Create new connection
        info!(service = %service.name, url = %url, "Creating projector client");
        let client = ProjectorCoordinatorClient::connect(url.clone())
            .await
            .map_err(|e| DiscoveryError::ConnectionFailed {
                service: service.name.clone(),
                address: url.clone(),
                message: e.to_string(),
            })?;

        // Cache
        self.projector_clients.write().await.insert(url, client.clone());

        Ok(client)
    }

    /// Get saga coordinator clients matching source domain.
    ///
    /// Returns clients for sagas that listen to events from the given domain.
    pub async fn get_sagas_for_source(
        &self,
        source_domain: &str,
    ) -> Result<Vec<SagaCoordinatorClient<Channel>>, DiscoveryError> {
        let sagas = self.sagas.read().await;

        let matching: Vec<_> = sagas
            .values()
            .filter(|s| s.source_domain.as_deref() == Some(source_domain))
            .cloned()
            .collect();

        drop(sagas);

        if matching.is_empty() {
            return Ok(vec![]);
        }

        let mut clients = Vec::with_capacity(matching.len());
        for service in matching {
            match self.get_or_create_saga_client(&service).await {
                Ok(client) => clients.push(client),
                Err(e) => warn!(service = %service.name, error = %e, "Failed to get saga client"),
            }
        }

        Ok(clients)
    }

    async fn get_or_create_saga_client(
        &self,
        service: &DiscoveredService,
    ) -> Result<SagaCoordinatorClient<Channel>, DiscoveryError> {
        let url = service.grpc_url();

        // Check cache
        {
            let clients = self.saga_clients.read().await;
            if let Some(client) = clients.get(&url) {
                debug!(service = %service.name, "Using cached saga client");
                return Ok(client.clone());
            }
        }

        // Create new connection
        info!(service = %service.name, url = %url, "Creating saga client");
        let client = SagaCoordinatorClient::connect(url.clone())
            .await
            .map_err(|e| DiscoveryError::ConnectionFailed {
                service: service.name.clone(),
                address: url.clone(),
                message: e.to_string(),
            })?;

        // Cache
        self.saga_clients.write().await.insert(url, client.clone());

        Ok(client)
    }

    /// Check if any aggregates are available.
    pub async fn has_aggregates(&self) -> bool {
        !self.aggregates.read().await.is_empty()
    }

    /// Check if any projectors are available.
    pub async fn has_projectors(&self) -> bool {
        !self.projectors.read().await.is_empty()
    }

    /// Check if any sagas are available.
    pub async fn has_sagas(&self) -> bool {
        !self.sagas.read().await.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{ServicePort, ServiceSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

    fn make_test_service(
        name: &str,
        component: &str,
        domain: Option<&str>,
        source_domain: Option<&str>,
        port: i32,
    ) -> Service {
        let mut labels = BTreeMap::new();
        labels.insert(COMPONENT_LABEL.to_string(), component.to_string());
        if let Some(d) = domain {
            labels.insert(DOMAIN_LABEL.to_string(), d.to_string());
        }
        if let Some(sd) = source_domain {
            labels.insert(SOURCE_DOMAIN_LABEL.to_string(), sd.to_string());
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
        let svc = make_test_service("cart-agg", COMPONENT_AGGREGATE, Some("cart"), None, 50051);
        let discovered = ServiceDiscovery::extract_service_with_namespace(&svc, "test-ns").unwrap();

        assert_eq!(discovered.name, "cart-agg");
        assert_eq!(discovered.service_address, "cart-agg.test-ns.svc.cluster.local");
        assert_eq!(discovered.port, 50051);
        assert_eq!(discovered.domain, Some("cart".to_string()));
        assert_eq!(discovered.source_domain, None);
    }

    #[test]
    fn test_extract_projector_service() {
        let svc = make_test_service("cart-proj", COMPONENT_PROJECTOR, Some("cart"), None, 50052);
        let discovered = ServiceDiscovery::extract_service_with_namespace(&svc, "test-ns").unwrap();

        assert_eq!(discovered.name, "cart-proj");
        assert_eq!(discovered.domain, Some("cart".to_string()));
    }

    #[test]
    fn test_extract_saga_service() {
        let svc = make_test_service(
            "order-saga",
            COMPONENT_SAGA,
            Some("fulfillment"),
            Some("order"),
            50053,
        );
        let discovered = ServiceDiscovery::extract_service_with_namespace(&svc, "test-ns").unwrap();

        assert_eq!(discovered.name, "order-saga");
        assert_eq!(discovered.domain, Some("fulfillment".to_string()));
        assert_eq!(discovered.source_domain, Some("order".to_string()));
    }

    #[test]
    fn test_grpc_url() {
        let service = DiscoveredService {
            name: "test-svc".to_string(),
            service_address: "test-svc.ns.svc.cluster.local".to_string(),
            port: 50051,
            domain: None,
            source_domain: None,
        };

        assert_eq!(service.grpc_url(), "http://test-svc.ns.svc.cluster.local:50051");
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

        let discovered = ServiceDiscovery::extract_service_with_namespace(&svc, "test-ns").unwrap();
        assert_eq!(discovered.port, DEFAULT_GRPC_PORT);
    }
}
