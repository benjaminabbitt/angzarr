//! Server and networking configuration types.

use serde::{Deserialize, Deserializer};

/// Deserialize optional args from either a Vec<String>, JSON string, or null.
fn deserialize_args_default<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ArgsOrString {
        Args(Vec<String>),
        JsonString(String),
        Null,
    }

    match Option::<ArgsOrString>::deserialize(deserializer)? {
        Some(ArgsOrString::Args(args)) => Ok(args),
        Some(ArgsOrString::JsonString(s)) if !s.is_empty() => {
            // Try to parse as JSON array
            serde_json::from_str(&s).map_err(serde::de::Error::custom)
        }
        _ => Ok(Vec::new()),
    }
}

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
    /// Additional arguments to pass to the command.
    /// If provided, command is treated as the executable and args as its arguments.
    /// If not provided, command is passed to `sh -c` for shell interpretation.
    #[serde(default, deserialize_with = "deserialize_args_default")]
    pub args: Vec<String>,
    /// Working directory for the spawned process.
    pub working_dir: Option<String>,
    /// Domains to listen for events from (saga/projector sidecars).
    /// Empty means all domains. Uses hierarchical matching.
    #[serde(default)]
    pub listen_domains: Vec<String>,
}

/// Configuration for a service in standalone mode.
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    /// Domain name (for aggregates) or handler name (for sagas/projectors).
    pub domain: String,
    /// Projector name (optional, for projectors with multiple instances per domain).
    #[serde(default)]
    pub name: Option<String>,
    /// Command to spawn the service.
    pub command: String,
    /// Additional arguments to pass to the command.
    /// If provided, command is treated as the executable and args as its arguments.
    /// If not provided, command is passed to `sh -c` for shell interpretation.
    #[serde(default, deserialize_with = "deserialize_args_default")]
    pub args: Vec<String>,
    /// Domains to listen for events from (sagas and projectors only).
    #[serde(default)]
    pub listen_domains: Vec<String>,
    /// Environment variables to set for the spawned process.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

/// Gateway configuration for standalone mode.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GatewayConfig {
    /// Enable the gateway.
    pub enabled: bool,
    /// Port for TCP gateway (if not using UDS).
    pub port: Option<u16>,
}

/// Health check configuration for external services.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HealthCheckConfig {
    /// No health check.
    #[default]
    None,
    /// HTTP GET health check.
    Http {
        /// URL to check (e.g., "http://localhost:8080/health").
        endpoint: String,
    },
    /// TCP connection health check.
    Tcp {
        /// Address to connect to (e.g., "localhost:8080").
        address: String,
    },
    /// gRPC health check.
    Grpc {
        /// gRPC address (e.g., "localhost:50051").
        address: String,
    },
}

/// Configuration for an external service (REST API, GraphQL, etc.).
///
/// External services are arbitrary processes that read projection data
/// and serve it to clients. They are started after projectors.
#[derive(Debug, Clone, Deserialize)]
pub struct ExternalServiceConfig {
    /// Service name (for logging and identification).
    pub name: String,
    /// Command to execute.
    pub command: String,
    /// Command arguments.
    #[serde(default, deserialize_with = "deserialize_args_default")]
    pub args: Vec<String>,
    /// Working directory.
    pub working_dir: Option<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Health check configuration.
    #[serde(default)]
    pub health_check: HealthCheckConfig,
    /// Health check timeout in seconds.
    #[serde(default = "default_health_timeout")]
    pub health_timeout_secs: u64,
}

fn default_health_timeout() -> u64 {
    30
}

/// Standalone mode configuration.
/// Groups all settings for running angzarr locally with spawned processes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct StandaloneConfig {
    /// Aggregate services (business logic handlers).
    pub aggregates: Vec<ServiceConfig>,
    /// Saga services (cross-aggregate workflows).
    pub sagas: Vec<ServiceConfig>,
    /// Projector services (read model builders).
    pub projectors: Vec<ServiceConfig>,
    /// External services (REST APIs, GraphQL servers, etc.).
    #[serde(default)]
    pub services: Vec<ExternalServiceConfig>,
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
