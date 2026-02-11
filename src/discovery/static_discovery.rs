//! Static service discovery with environment variable configuration.
//!
//! Provides service discovery without K8s dependencies. Services are registered
//! manually or loaded from environment variables at startup.
//!
//! # Environment Variable Configuration
//!
//! ```bash
//! # Aggregates: ANGZARR_AGGREGATE_{DOMAIN}=url
//! ANGZARR_AGGREGATE_ORDER=https://order-coordinator.run.app
//! ANGZARR_AGGREGATE_INVENTORY=https://inventory-coordinator.run.app
//!
//! # Projectors: JSON array
//! ANGZARR_PROJECTORS='[{"name":"web","domain":"order","url":"https://web-projector.run.app"}]'
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::RwLock;
use tonic::transport::Channel;
use tracing::{debug, info, warn};

use crate::config::EVENT_QUERY_ADDRESS_ENV_VAR;
use crate::proto::aggregate_coordinator_service_client::AggregateCoordinatorServiceClient;
use crate::proto::event_query_service_client::EventQueryServiceClient;
use crate::proto::projector_coordinator_service_client::ProjectorCoordinatorServiceClient;
use crate::proto_ext::WILDCARD_DOMAIN;

use super::{DiscoveredService, DiscoveryError};

/// Environment variable prefix for aggregate URLs.
const AGGREGATE_PREFIX: &str = "ANGZARR_AGGREGATE_";

/// Environment variable for projector JSON array.
const PROJECTORS_VAR: &str = "ANGZARR_PROJECTORS";

/// Projector entry from JSON configuration.
#[derive(Debug, Deserialize)]
struct ProjectorEntry {
    name: String,
    domain: String,
    url: String,
}

/// Static service discovery without K8s dependencies.
///
/// Services are registered manually via `register_aggregate`/`register_projector`
/// or loaded from environment variables via `from_env()`.
pub struct StaticServiceDiscovery {
    #[allow(dead_code)] // Reserved for future logging/debugging
    namespace: String,
    aggregates: Arc<RwLock<HashMap<String, DiscoveredService>>>,
    projectors: Arc<RwLock<HashMap<String, DiscoveredService>>>,
    aggregate_clients: Arc<RwLock<HashMap<String, AggregateCoordinatorServiceClient<Channel>>>>,
    event_query_clients: Arc<RwLock<HashMap<String, EventQueryServiceClient<Channel>>>>,
    projector_clients: Arc<RwLock<HashMap<String, ProjectorCoordinatorServiceClient<Channel>>>>,
}

impl StaticServiceDiscovery {
    /// Create a new empty static discovery instance.
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            aggregates: empty_cache(),
            projectors: empty_cache(),
            aggregate_clients: empty_cache(),
            event_query_clients: empty_cache(),
            projector_clients: empty_cache(),
        }
    }

    /// Create from environment variables.
    ///
    /// Scans for `ANGZARR_AGGREGATE_*` and `ANGZARR_PROJECTORS` env vars.
    pub fn from_env() -> Self {
        let discovery = Self::new("env");

        // Parse aggregates: ANGZARR_AGGREGATE_{DOMAIN}=url
        for (key, value) in std::env::vars() {
            if let Some(domain) = key.strip_prefix(AGGREGATE_PREFIX) {
                let domain = domain.to_lowercase();
                if let Some((address, port)) = parse_url(&value) {
                    discovery.register_aggregate_sync(&domain, &address, port);
                } else {
                    warn!(
                        key = %key,
                        value = %value,
                        "Failed to parse aggregate URL"
                    );
                }
            }
        }

        // Parse projectors: ANGZARR_PROJECTORS='[{"name":"...","domain":"...","url":"..."}]'
        if let Ok(json) = std::env::var(PROJECTORS_VAR) {
            match serde_json::from_str::<Vec<ProjectorEntry>>(&json) {
                Ok(entries) => {
                    for entry in entries {
                        if let Some((address, port)) = parse_url(&entry.url) {
                            discovery.register_projector_sync(
                                &entry.name,
                                &entry.domain,
                                &address,
                                port,
                            );
                        } else {
                            warn!(
                                name = %entry.name,
                                url = %entry.url,
                                "Failed to parse projector URL"
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        "Failed to parse ANGZARR_PROJECTORS JSON"
                    );
                }
            }
        }

        let agg_count = discovery.aggregates.blocking_read().len();
        let proj_count = discovery.projectors.blocking_read().len();
        info!(
            aggregates = agg_count,
            projectors = proj_count,
            "Static discovery initialized from environment"
        );

        discovery
    }

    /// Register an aggregate synchronously (for use in constructors).
    pub fn register_aggregate_sync(&self, domain: &str, address: &str, port: u16) {
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
            .blocking_write()
            .insert(service.name.clone(), service);
    }

    /// Register a projector synchronously (for use in constructors).
    pub fn register_projector_sync(&self, name: &str, domain: &str, address: &str, port: u16) {
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
            .blocking_write()
            .insert(service.name.clone(), service);
    }

    async fn get_or_create_aggregate_client(
        &self,
        service: &DiscoveredService,
    ) -> Result<AggregateCoordinatorServiceClient<Channel>, DiscoveryError> {
        get_or_create_client(
            &self.aggregate_clients,
            service,
            "aggregate",
            AggregateCoordinatorServiceClient::new,
        )
        .await
    }

    async fn get_or_create_event_query_client(
        &self,
        service: &DiscoveredService,
    ) -> Result<EventQueryServiceClient<Channel>, DiscoveryError> {
        get_or_create_client(
            &self.event_query_clients,
            service,
            "event_query",
            EventQueryServiceClient::new,
        )
        .await
    }

    async fn get_or_create_projector_client(
        &self,
        service: &DiscoveredService,
    ) -> Result<ProjectorCoordinatorServiceClient<Channel>, DiscoveryError> {
        get_or_create_client(
            &self.projector_clients,
            service,
            "projector",
            ProjectorCoordinatorServiceClient::new,
        )
        .await
    }
}

#[async_trait::async_trait]
impl super::ServiceDiscovery for StaticServiceDiscovery {
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
    ) -> Result<AggregateCoordinatorServiceClient<Channel>, DiscoveryError> {
        let aggregates = self.aggregates.read().await;

        // Find service matching domain, or wildcard
        let service = aggregates
            .values()
            .find(|s| s.domain.as_deref() == Some(domain))
            .or_else(|| {
                aggregates
                    .values()
                    .find(|s| s.domain.as_deref() == Some(WILDCARD_DOMAIN))
            })
            .ok_or_else(|| DiscoveryError::DomainNotFound(domain.to_string()))?
            .clone();

        drop(aggregates);

        self.get_or_create_aggregate_client(&service).await
    }

    async fn get_event_query(
        &self,
        domain: &str,
    ) -> Result<EventQueryServiceClient<Channel>, DiscoveryError> {
        let aggregates = self.aggregates.read().await;

        // Find service matching domain, or wildcard
        let service = aggregates
            .values()
            .find(|s| s.domain.as_deref() == Some(domain))
            .or_else(|| {
                aggregates
                    .values()
                    .find(|s| s.domain.as_deref() == Some(WILDCARD_DOMAIN))
            })
            .cloned();

        drop(aggregates);

        if let Some(service) = service {
            return self.get_or_create_event_query_client(&service).await;
        }

        // Fallback to EVENT_QUERY_ADDRESS_ENV_VAR env var
        if let Ok(addr) = std::env::var(EVENT_QUERY_ADDRESS_ENV_VAR) {
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
    ) -> Result<Vec<ProjectorCoordinatorServiceClient<Channel>>, DiscoveryError> {
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

    async fn get_projector_by_name(
        &self,
        name: &str,
    ) -> Result<ProjectorCoordinatorServiceClient<Channel>, DiscoveryError> {
        let projectors = self.projectors.read().await;

        let service = projectors
            .values()
            .find(|s| s.name == name || s.domain.as_deref() == Some(name))
            .ok_or_else(|| DiscoveryError::NoServicesFound(format!("projector:{}", name)))?
            .clone();

        drop(projectors);

        self.get_or_create_projector_client(&service).await
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
        // No-op for static discovery - services are registered manually
        Ok(())
    }

    fn start_watching(&self) {
        // No-op for static discovery - no background watching
    }
}

/// Get or create a cached gRPC client connection.
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

/// Parse a URL into (address, port).
///
/// Handles:
/// - UDS paths: `/path/to/socket` -> ("/path/to/socket", 0)
/// - HTTP URLs: `http://host:port` -> ("host", port)
/// - HTTPS URLs: `https://host:port` -> ("host", port)
/// - Plain host:port: `host:8080` -> ("host", 8080)
fn parse_url(url: &str) -> Option<(String, u16)> {
    if url.starts_with('/') {
        // UDS path
        Some((url.to_string(), 0))
    } else if url.starts_with("http://") || url.starts_with("https://") {
        // Parse as URL
        let without_scheme = url
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        // Remove path if present
        let host_port = without_scheme.split('/').next()?;
        if let Some((host, port_str)) = host_port.rsplit_once(':') {
            let port = port_str.parse().ok()?;
            Some((host.to_string(), port))
        } else {
            // No port, use 443 for https, 80 for http
            let port = if url.starts_with("https://") { 443 } else { 80 };
            Some((host_port.to_string(), port))
        }
    } else if let Some((host, port_str)) = url.rsplit_once(':') {
        // Plain host:port
        let port = port_str.parse().ok()?;
        Some((host.to_string(), port))
    } else {
        // Just a host, no port
        Some((url.to_string(), 80))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::ServiceDiscovery;

    #[test]
    fn test_parse_url_uds() {
        let (addr, port) = parse_url("/tmp/angzarr/test.sock").unwrap();
        assert_eq!(addr, "/tmp/angzarr/test.sock");
        assert_eq!(port, 0);
    }

    #[test]
    fn test_parse_url_http() {
        let (addr, port) = parse_url("http://localhost:8080").unwrap();
        assert_eq!(addr, "localhost");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_url_https() {
        let (addr, port) = parse_url("https://order-coordinator.run.app").unwrap();
        assert_eq!(addr, "order-coordinator.run.app");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_url_https_with_port() {
        let (addr, port) = parse_url("https://order-coordinator.run.app:8443").unwrap();
        assert_eq!(addr, "order-coordinator.run.app");
        assert_eq!(port, 8443);
    }

    #[test]
    fn test_parse_url_host_port() {
        let (addr, port) = parse_url("localhost:50051").unwrap();
        assert_eq!(addr, "localhost");
        assert_eq!(port, 50051);
    }

    #[test]
    fn test_parse_url_with_path() {
        let (addr, port) = parse_url("https://order-coordinator.run.app/api/v1").unwrap();
        assert_eq!(addr, "order-coordinator.run.app");
        assert_eq!(port, 443);
    }

    #[tokio::test]
    async fn test_static_discovery_register_aggregate() {
        let discovery = StaticServiceDiscovery::new("test");
        discovery
            .register_aggregate("order", "localhost", 50051)
            .await;

        assert!(discovery.has_aggregates().await);
        let domains = discovery.aggregate_domains().await;
        assert_eq!(domains, vec!["order"]);
    }

    #[tokio::test]
    async fn test_static_discovery_register_projector() {
        let discovery = StaticServiceDiscovery::new("test");
        discovery
            .register_projector("web", "order", "localhost", 50052)
            .await;

        assert!(discovery.has_projectors().await);
    }

    #[test]
    fn test_static_discovery_sync_registration() {
        let discovery = StaticServiceDiscovery::new("test");
        discovery.register_aggregate_sync("order", "localhost", 50051);
        discovery.register_projector_sync("web", "order", "localhost", 50052);

        assert!(!discovery.aggregates.blocking_read().is_empty());
        assert!(!discovery.projectors.blocking_read().is_empty());
    }
}
