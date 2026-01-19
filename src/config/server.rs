//! Server and networking configuration types.

use serde::Deserialize;

/// Server configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Port for aggregate gRPC service.
    pub aggregate_port: u16,
    /// Port for event query gRPC service.
    pub event_query_port: u16,
    /// Host to bind to.
    pub host: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            aggregate_port: 1313,
            event_query_port: 1314,
            host: "0.0.0.0".to_string(),
        }
    }
}

/// Target service configuration for sidecar modes.
#[derive(Debug, Clone, Deserialize)]
pub struct TargetConfig {
    /// gRPC address of the target service.
    pub address: String,
    /// Domain handled by this service (for aggregate mode).
    pub domain: Option<String>,
    /// Command to spawn the business logic process.
    /// If set, the sidecar will spawn this process before connecting.
    pub command: Option<String>,
    /// Working directory for the spawned process.
    pub working_dir: Option<String>,
}

/// Configuration for a service in embedded mode.
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    /// Domain name (for aggregates) or handler name (for sagas/projectors).
    pub domain: String,
    /// Projector name (optional, for projectors with multiple instances per domain).
    #[serde(default)]
    pub name: Option<String>,
    /// Command to spawn the service.
    pub command: String,
    /// Domains to listen for events from (sagas and projectors only).
    #[serde(default)]
    pub listen_domains: Vec<String>,
}

/// Gateway configuration for embedded mode.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GatewayConfig {
    /// Enable the gateway.
    pub enabled: bool,
    /// Port for TCP gateway (if not using UDS).
    pub port: Option<u16>,
}

/// Embedded mode configuration.
/// Groups all settings for running angzarr locally with spawned processes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct EmbeddedConfig {
    /// Aggregate services (business logic handlers).
    pub aggregates: Vec<ServiceConfig>,
    /// Saga services (cross-aggregate workflows).
    pub sagas: Vec<ServiceConfig>,
    /// Projector services (read model builders).
    pub projectors: Vec<ServiceConfig>,
    /// Gateway configuration.
    pub gateway: GatewayConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let server = ServerConfig::default();
        assert_eq!(server.aggregate_port, 1313);
        assert_eq!(server.event_query_port, 1314);
        assert_eq!(server.host, "0.0.0.0");
    }
}
