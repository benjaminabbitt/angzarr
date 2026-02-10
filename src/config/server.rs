//! Server and networking configuration types.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::storage::StorageConfig;
use crate::transport::TransportConfig;

/// Server configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Port for aggregate gRPC service.
    pub aggregate_port: u16,
    /// Port for event query gRPC service.
    pub event_query_port: u16,
    /// Host to bind to.
    ///
    /// Default is `127.0.0.1` (localhost only) for security.
    /// Set to `0.0.0.0` explicitly to bind to all interfaces.
    pub host: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            aggregate_port: 1313,
            event_query_port: 1314,
            // Default to localhost for security - external access requires explicit config
            host: "127.0.0.1".to_string(),
        }
    }
}

/// Unified service configuration for both sidecar and standalone modes.
///
/// Works as:
/// - A sidecar target: `target: { domain: cart, command: [...] }`
/// - A standalone entry: `standalone.aggregates[*]`
/// - A file reference: `{ file: path/to/service.yaml }`
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    /// Domain name (for aggregates) or handler name (for sagas/projectors).
    pub domain: String,

    /// Service name (optional, for projectors with multiple instances per domain).
    #[serde(default)]
    pub name: Option<String>,

    /// Explicit gRPC address override.
    /// If not set, derived from transport config:
    /// - UDS: `{base_path}/{service_type}-{domain}.sock`
    /// - TCP: Must be explicit
    #[serde(default)]
    pub address: Option<String>,

    /// Working directory for the spawned process.
    #[serde(default)]
    pub working_dir: Option<String>,

    /// Command to spawn the service as array: ["executable", "arg1", "arg2", ...].
    /// First element is the executable, rest are arguments. No shell interpretation.
    #[serde(default)]
    pub command: Vec<String>,

    /// Domain to listen for events from (sagas and projectors only).
    #[serde(default)]
    pub listen_domain: Option<String>,

    /// Environment variables to set for the spawned process.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Per-service storage configuration.
    /// If not set, falls back to the root storage config.
    #[serde(default)]
    pub storage: Option<StorageConfig>,
}

impl ServiceConfig {
    /// Resolve the address for this service.
    ///
    /// Uses explicit address if set, otherwise derives from transport config.
    ///
    /// # Errors
    /// Returns error if TCP transport is used without an explicit address.
    pub fn resolve_address(
        &self,
        transport: &TransportConfig,
        service_type: &str,
    ) -> Result<String, ConfigError> {
        use crate::transport::TransportType;

        if let Some(ref addr) = self.address {
            return Ok(addr.clone());
        }

        match transport.transport_type {
            TransportType::Uds => {
                let socket_name = match &self.name {
                    Some(name) => format!("{}-{}-{}", service_type, name, self.domain),
                    None => format!("{}-{}", service_type, self.domain),
                };
                Ok(format!(
                    "{}/{}.sock",
                    transport.uds.base_path.display(),
                    socket_name
                ))
            }
            TransportType::Tcp => Err(ConfigError::Parse(
                self.domain.clone(),
                "TCP transport requires explicit address".to_string(),
            )),
        }
    }
}

/// Backwards-compatible alias for sidecar mode.
pub type TargetConfig = ServiceConfig;

/// Fields that can be overridden when referencing a file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ServiceConfigOverrides {
    /// Override storage configuration.
    #[serde(default)]
    pub storage: Option<StorageConfig>,
    /// Additional environment variables (merged with file's env).
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    /// Override working directory.
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Override address.
    #[serde(default)]
    pub address: Option<String>,
}

/// Service configuration that can be inline or a file reference.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ServiceConfigRef {
    /// File reference with optional overrides.
    File {
        /// Path to external config file (relative to main config).
        file: PathBuf,
        /// Override fields from the referenced file.
        #[serde(flatten)]
        overrides: ServiceConfigOverrides,
    },
    /// Inline service configuration.
    Inline(ServiceConfig),
}

impl ServiceConfigRef {
    /// Resolve to a concrete ServiceConfig, loading from file if needed.
    pub fn resolve(&self, base_dir: &Path) -> Result<ServiceConfig, ConfigError> {
        match self {
            ServiceConfigRef::Inline(config) => Ok(config.clone()),
            ServiceConfigRef::File { file, overrides } => {
                let path = base_dir.join(file);
                let content = std::fs::read_to_string(&path).map_err(|e| {
                    ConfigError::FileRead(path.display().to_string(), e.to_string())
                })?;
                let mut config: ServiceConfig = serde_yaml::from_str(&content)
                    .map_err(|e| ConfigError::Parse(path.display().to_string(), e.to_string()))?;

                // Apply overrides
                if let Some(storage) = &overrides.storage {
                    config.storage = Some(storage.clone());
                }
                if let Some(env) = &overrides.env {
                    config.env.extend(env.clone());
                }
                if let Some(working_dir) = &overrides.working_dir {
                    config.working_dir = Some(working_dir.clone());
                }
                if let Some(address) = &overrides.address {
                    config.address = Some(address.clone());
                }

                Ok(config)
            }
        }
    }
}

/// Error type for configuration loading.
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// Failed to read config file.
    FileRead(String, String),
    /// Failed to parse config file.
    Parse(String, String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::FileRead(path, err) => write!(f, "Failed to read '{}': {}", path, err),
            ConfigError::Parse(path, err) => write!(f, "Failed to parse '{}': {}", path, err),
        }
    }
}

impl std::error::Error for ConfigError {}

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
    /// Command to spawn as array: ["executable", "arg1", "arg2", ...].
    /// First element is the executable, rest are arguments. No shell interpretation.
    pub command: Vec<String>,
    /// Working directory.
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
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

/// Configuration for component registration/republishing.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RegistrationConfig {
    /// Republish strategy: "fixed" or "exponential".
    #[serde(default = "default_registration_strategy")]
    pub strategy: String,
    /// Fixed interval in seconds (used when strategy = "fixed").
    #[serde(default = "default_registration_interval")]
    pub interval_secs: u64,
    /// Initial delay in seconds for exponential backoff.
    #[serde(default = "default_registration_initial")]
    pub initial_secs: u64,
    /// Maximum delay in seconds for exponential backoff.
    #[serde(default = "default_registration_max")]
    pub max_secs: u64,
    /// Backoff multiplier for exponential strategy.
    #[serde(default = "default_registration_multiplier")]
    pub multiplier: f64,
    /// Enable jitter for exponential backoff.
    #[serde(default = "default_registration_jitter")]
    pub jitter: bool,
}

fn default_registration_strategy() -> String {
    "fixed".to_string()
}

fn default_registration_interval() -> u64 {
    30
}

fn default_registration_initial() -> u64 {
    1
}

fn default_registration_max() -> u64 {
    60
}

fn default_registration_multiplier() -> f64 {
    2.0
}

fn default_registration_jitter() -> bool {
    true
}

impl Default for RegistrationConfig {
    fn default() -> Self {
        Self {
            strategy: default_registration_strategy(),
            interval_secs: default_registration_interval(),
            initial_secs: default_registration_initial(),
            max_secs: default_registration_max(),
            multiplier: default_registration_multiplier(),
            jitter: default_registration_jitter(),
        }
    }
}

impl RegistrationConfig {
    /// Build a republish strategy from this config.
    pub fn build_strategy(&self) -> Box<dyn crate::registration::RepublishStrategy> {
        use crate::registration::{ExponentialBackoff, FixedInterval};
        use std::time::Duration;

        match self.strategy.as_str() {
            "exponential" => Box::new(
                ExponentialBackoff::new()
                    .with_initial(Duration::from_secs(self.initial_secs))
                    .with_max(Duration::from_secs(self.max_secs))
                    .with_multiplier(self.multiplier)
                    .with_jitter(self.jitter),
            ),
            _ => Box::new(FixedInterval::new(Duration::from_secs(self.interval_secs))),
        }
    }
}

/// Standalone mode configuration.
/// Groups all settings for running angzarr locally with spawned processes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct StandaloneConfig {
    /// Aggregate services (client logic handlers).
    pub aggregates: Vec<ServiceConfig>,
    /// Saga services (cross-aggregate workflows).
    pub sagas: Vec<ServiceConfig>,
    /// Process manager services (stateful workflow coordinators).
    pub process_managers: Vec<ServiceConfig>,
    /// Projector services (read model builders).
    pub projectors: Vec<ServiceConfig>,
    /// External services (REST APIs, GraphQL servers, etc.).
    #[serde(default)]
    pub services: Vec<ExternalServiceConfig>,
    /// Component registration/republishing configuration.
    #[serde(default)]
    pub registration: RegistrationConfig,
    /// Gateway configuration.
    pub gateway: GatewayConfig,
}

/// Resolved standalone config with all file references expanded.
#[derive(Debug, Clone)]
pub struct ResolvedStandaloneConfig {
    /// Resolved aggregate services.
    pub aggregates: Vec<ServiceConfig>,
    /// Resolved saga services.
    pub sagas: Vec<ServiceConfig>,
    /// Resolved process manager services.
    pub process_managers: Vec<ServiceConfig>,
    /// Resolved projector services.
    pub projectors: Vec<ServiceConfig>,
    /// External services (no resolution needed).
    pub services: Vec<ExternalServiceConfig>,
    /// Component registration/republishing configuration.
    pub registration: RegistrationConfig,
    /// Gateway configuration.
    pub gateway: GatewayConfig,
}

impl StandaloneConfig {
    /// Resolve all service references, loading external files.
    /// Currently a no-op since StandaloneConfig uses direct ServiceConfig.
    /// File references support will be added later via ServiceConfigRef.
    pub fn resolve(&self, _base_dir: &Path) -> Result<ResolvedStandaloneConfig, ConfigError> {
        Ok(ResolvedStandaloneConfig {
            aggregates: self.aggregates.clone(),
            sagas: self.sagas.clone(),
            process_managers: self.process_managers.clone(),
            projectors: self.projectors.clone(),
            services: self.services.clone(),
            registration: self.registration.clone(),
            gateway: self.gateway.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let server = ServerConfig::default();
        assert_eq!(server.aggregate_port, 1313);
        assert_eq!(server.event_query_port, 1314);
        // Default to localhost for security
        assert_eq!(server.host, "127.0.0.1");
    }

    #[test]
    fn test_service_config_inline_deserialization() {
        let yaml = r#"
            domain: cart
            command: ["python", "server.py"]
        "#;
        let config: ServiceConfigRef = serde_yaml::from_str(yaml).unwrap();
        match config {
            ServiceConfigRef::Inline(svc) => {
                assert_eq!(svc.domain, "cart");
                assert_eq!(svc.command, vec!["python", "server.py"]);
            }
            ServiceConfigRef::File { .. } => panic!("Expected inline config"),
        }
    }

    #[test]
    fn test_service_config_file_ref_deserialization() {
        let yaml = r#"
            file: config/cart.yaml
            storage:
              type: sqlite
        "#;
        let config: ServiceConfigRef = serde_yaml::from_str(yaml).unwrap();
        match config {
            ServiceConfigRef::File { file, overrides } => {
                assert_eq!(file, PathBuf::from("config/cart.yaml"));
                assert!(overrides.storage.is_some());
            }
            ServiceConfigRef::Inline(_) => panic!("Expected file reference"),
        }
    }
}
