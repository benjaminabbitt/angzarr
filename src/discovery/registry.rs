//! Service registry - thread-safe storage of discovered services with connection pooling.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tonic::transport::Channel;
use tracing::{debug, info, warn};

use crate::proto::business_coordinator_client::BusinessCoordinatorClient;

/// A discovered service endpoint.
#[derive(Debug, Clone)]
pub struct ServiceEndpoint {
    pub domain: String,
    pub address: String,
    pub port: u16,
}

impl ServiceEndpoint {
    /// Get the full address string for gRPC connection.
    pub fn grpc_address(&self) -> String {
        format!("http://{}:{}", self.address, self.port)
    }
}

/// Error types for service registry operations.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Domain not found: {0}")]
    DomainNotFound(String),

    #[error("Connection failed to {domain} at {address}: {message}")]
    ConnectionFailed {
        domain: String,
        address: String,
        message: String,
    },
}

/// Thread-safe service registry with connection pooling.
///
/// Maintains a map of domain -> endpoint and lazily creates gRPC connections
/// on first request to each domain.
pub struct ServiceRegistry {
    endpoints: Arc<RwLock<HashMap<String, ServiceEndpoint>>>,
    connections: Arc<RwLock<HashMap<String, BusinessCoordinatorClient<Channel>>>>,
}

impl ServiceRegistry {
    /// Create a new empty service registry.
    pub fn new() -> Self {
        Self {
            endpoints: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update or add an endpoint for a domain.
    pub async fn update_endpoint(&self, endpoint: ServiceEndpoint) {
        let domain = endpoint.domain.clone();
        let address = endpoint.grpc_address();

        // Check if endpoint changed
        let mut endpoints = self.endpoints.write().await;
        let endpoint_changed = endpoints
            .get(&domain)
            .map(|e| e.grpc_address() != address)
            .unwrap_or(true);

        if endpoint_changed {
            info!(
                domain = %domain,
                address = %address,
                "Registering service endpoint"
            );

            // Remove stale connection if endpoint changed
            let mut connections = self.connections.write().await;
            if connections.remove(&domain).is_some() {
                debug!(domain = %domain, "Removed stale connection");
            }
            drop(connections);

            endpoints.insert(domain, endpoint);
        }
    }

    /// Remove an endpoint and its connection.
    pub async fn remove_endpoint(&self, domain: &str) {
        info!(domain = %domain, "Removing service endpoint");

        let mut endpoints = self.endpoints.write().await;
        endpoints.remove(domain);
        drop(endpoints);

        let mut connections = self.connections.write().await;
        connections.remove(domain);
    }

    /// Get or create a client for the specified domain.
    ///
    /// Returns a clone of the cached client, or creates a new connection
    /// if one doesn't exist. Supports wildcard domain "*" as fallback.
    pub async fn get_client(
        &self,
        domain: &str,
    ) -> Result<BusinessCoordinatorClient<Channel>, RegistryError> {
        // Try exact domain match first, then wildcard
        let lookup_domain = {
            let endpoints = self.endpoints.read().await;
            if endpoints.contains_key(domain) {
                domain.to_string()
            } else if endpoints.contains_key("*") {
                "*".to_string()
            } else {
                return Err(RegistryError::DomainNotFound(domain.to_string()));
            }
        };

        // Check for existing connection
        {
            let connections = self.connections.read().await;
            if let Some(client) = connections.get(&lookup_domain) {
                debug!(domain = %domain, lookup = %lookup_domain, "Using cached connection");
                return Ok(client.clone());
            }
        }

        // Need to create new connection
        let endpoint = {
            let endpoints = self.endpoints.read().await;
            endpoints
                .get(&lookup_domain)
                .cloned()
                .ok_or_else(|| RegistryError::DomainNotFound(domain.to_string()))?
        };

        let address = endpoint.grpc_address();
        info!(
            domain = %domain,
            lookup = %lookup_domain,
            address = %address,
            "Creating new connection"
        );

        let client = BusinessCoordinatorClient::connect(address.clone())
            .await
            .map_err(|e| {
                warn!(
                    domain = %domain,
                    address = %address,
                    error = %e,
                    "Connection failed"
                );
                RegistryError::ConnectionFailed {
                    domain: domain.to_string(),
                    address,
                    message: e.to_string(),
                }
            })?;

        // Cache the connection
        let mut connections = self.connections.write().await;
        connections.insert(lookup_domain.clone(), client.clone());

        Ok(client)
    }

    /// Get list of all registered domains.
    pub async fn domains(&self) -> Vec<String> {
        let endpoints = self.endpoints.read().await;
        endpoints.keys().cloned().collect()
    }

    /// Check if a domain is registered.
    pub async fn has_domain(&self, domain: &str) -> bool {
        let endpoints = self.endpoints.read().await;
        endpoints.contains_key(domain) || endpoints.contains_key("*")
    }

    /// Get endpoint for a domain (without creating a connection).
    ///
    /// Returns the endpoint info for the domain, or wildcard if registered.
    pub async fn get_endpoint(&self, domain: &str) -> Result<ServiceEndpoint, RegistryError> {
        let endpoints = self.endpoints.read().await;

        // Try exact domain match first, then wildcard
        if let Some(endpoint) = endpoints.get(domain) {
            return Ok(endpoint.clone());
        }
        if let Some(endpoint) = endpoints.get("*") {
            return Ok(endpoint.clone());
        }

        Err(RegistryError::DomainNotFound(domain.to_string()))
    }

    /// Get the number of registered endpoints.
    pub async fn len(&self) -> usize {
        let endpoints = self.endpoints.read().await;
        endpoints.len()
    }

    /// Check if the registry is empty.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_add_and_lookup_endpoint() {
        let registry = ServiceRegistry::new();

        registry
            .update_endpoint(ServiceEndpoint {
                domain: "customer".to_string(),
                address: "localhost".to_string(),
                port: 50051,
            })
            .await;

        assert!(registry.has_domain("customer").await);
        assert!(!registry.has_domain("unknown").await);

        let domains = registry.domains().await;
        assert_eq!(domains, vec!["customer"]);
    }

    #[tokio::test]
    async fn test_registry_remove_endpoint() {
        let registry = ServiceRegistry::new();

        registry
            .update_endpoint(ServiceEndpoint {
                domain: "customer".to_string(),
                address: "localhost".to_string(),
                port: 50051,
            })
            .await;

        assert!(registry.has_domain("customer").await);

        registry.remove_endpoint("customer").await;

        assert!(!registry.has_domain("customer").await);
        assert!(registry.is_empty().await);
    }

    #[tokio::test]
    async fn test_registry_wildcard_fallback() {
        let registry = ServiceRegistry::new();

        // Register wildcard endpoint
        registry
            .update_endpoint(ServiceEndpoint {
                domain: "*".to_string(),
                address: "localhost".to_string(),
                port: 50051,
            })
            .await;

        // Unknown domain should match wildcard
        assert!(registry.has_domain("anything").await);
        assert!(registry.has_domain("customer").await);
    }

    #[tokio::test]
    async fn test_registry_domain_not_found() {
        let registry = ServiceRegistry::new();

        let result = registry.get_client("nonexistent").await;
        assert!(matches!(result, Err(RegistryError::DomainNotFound(_))));
    }

    #[tokio::test]
    async fn test_registry_update_removes_stale_connection() {
        let registry = ServiceRegistry::new();

        // Add initial endpoint
        registry
            .update_endpoint(ServiceEndpoint {
                domain: "customer".to_string(),
                address: "old-host".to_string(),
                port: 50051,
            })
            .await;

        // Update with new address
        registry
            .update_endpoint(ServiceEndpoint {
                domain: "customer".to_string(),
                address: "new-host".to_string(),
                port: 50052,
            })
            .await;

        // Verify endpoint was updated
        let endpoints = registry.endpoints.read().await;
        let endpoint = endpoints.get("customer").unwrap();
        assert_eq!(endpoint.address, "new-host");
        assert_eq!(endpoint.port, 50052);
    }
}
