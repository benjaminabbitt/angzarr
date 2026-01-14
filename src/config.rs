//! Configuration for angzarr server.
//!
//! Supports YAML file and environment variable overrides.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Angzarr operation mode.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Standalone server mode (legacy, for backward compatibility).
    #[default]
    Standalone,
    /// Aggregate sidecar: receives commands, calls business logic, stores/publishes events.
    Aggregate,
    /// Projector sidecar: subscribes to events, calls projector gRPC.
    Projector,
    /// Saga sidecar: subscribes to events, calls saga gRPC, publishes commands.
    Saga,
}

/// Server configuration.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Operation mode.
    pub mode: Mode,
    /// Storage configuration.
    pub storage: StorageConfig,
    /// Server port configuration.
    pub server: ServerConfig,
    /// Saga compensation configuration.
    pub saga_compensation: SagaCompensationConfig,
    /// AMQP configuration (for sidecar modes).
    pub amqp: Option<AmqpConfig>,
    /// Target service address (for sidecar modes).
    pub target: Option<TargetConfig>,
    /// Business logic service endpoints (standalone mode).
    pub business_logic: Vec<BusinessLogicEndpoint>,
    /// Projector endpoints (standalone mode).
    pub projectors: Vec<ProjectorEndpoint>,
    /// Saga endpoints (standalone mode).
    pub sagas: Vec<SagaEndpoint>,
}

/// Storage configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Storage type: "sqlite" or "mongodb".
    #[serde(rename = "type")]
    pub storage_type: String,
    /// Path to database file (SQLite) or connection URI (MongoDB).
    pub path: String,
    /// Database name (MongoDB only).
    pub database: Option<String>,
    /// Snapshot enable/disable flags for debugging and troubleshooting.
    pub snapshots_enable: SnapshotsEnableConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: "sqlite".to_string(),
            path: "./data/events.db".to_string(),
            database: None,
            snapshots_enable: SnapshotsEnableConfig::default(),
        }
    }
}

/// Snapshot enable/disable configuration.
///
/// These flags are useful for debugging and troubleshooting snapshot-related issues.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SnapshotsEnableConfig {
    /// Enable reading snapshots when loading aggregate state.
    /// When false, always replays all events from the beginning.
    /// Useful for debugging to verify event replay produces correct state.
    /// Default: true
    pub read: bool,
    /// Enable writing snapshots after processing commands.
    /// When false, no snapshots are stored (pure event sourcing).
    /// Useful for troubleshooting snapshot persistence issues.
    /// Default: true
    pub write: bool,
}

impl Default for SnapshotsEnableConfig {
    fn default() -> Self {
        Self {
            read: true,
            write: true,
        }
    }
}

/// Server configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Port for command handler gRPC service.
    pub command_handler_port: u16,
    /// Port for event query gRPC service.
    pub event_query_port: u16,
    /// Host to bind to.
    pub host: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            command_handler_port: 1313,
            event_query_port: 1314,
            host: "0.0.0.0".to_string(),
        }
    }
}

/// Business logic service endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct BusinessLogicEndpoint {
    /// Domain this service handles.
    pub domain: String,
    /// gRPC address.
    pub address: String,
}

/// Projector endpoint.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProjectorEndpoint {
    /// Name of the projector.
    pub name: String,
    /// gRPC address.
    pub address: String,
    /// If true, wait for response before continuing.
    pub synchronous: bool,
}

/// Saga endpoint.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SagaEndpoint {
    /// Name of the saga.
    pub name: String,
    /// gRPC address.
    pub address: String,
    /// If true, wait for response before continuing.
    pub synchronous: bool,
}

/// Saga compensation configuration.
///
/// Controls how saga command rejections are handled.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SagaCompensationConfig {
    /// Domain for fallback events when business logic cannot handle revocation.
    /// Default: "angzarr.saga-failures"
    pub fallback_domain: String,
    /// Dead letter queue URL (AMQP). None = DLQ disabled.
    pub dead_letter_queue_url: Option<String>,
    /// Webhook URL for escalation alerts. None = log only.
    pub escalation_webhook_url: Option<String>,
    /// Emit SagaCompensationFailed event on fallback (empty response or gRPC error).
    pub fallback_emit_system_revocation: bool,
    /// Send to DLQ on fallback.
    pub fallback_send_to_dlq: bool,
    /// Trigger escalation on fallback.
    pub fallback_escalate: bool,
}

impl Default for SagaCompensationConfig {
    fn default() -> Self {
        Self {
            fallback_domain: "angzarr.saga-failures".to_string(),
            dead_letter_queue_url: None,
            escalation_webhook_url: None,
            fallback_emit_system_revocation: true,
            fallback_send_to_dlq: false,
            fallback_escalate: false,
        }
    }
}

/// AMQP configuration for sidecar modes.
#[derive(Debug, Clone, Deserialize)]
pub struct AmqpConfig {
    /// AMQP connection URL.
    pub url: String,
    /// Domain to subscribe to (for aggregate mode, this is the command queue).
    pub domain: Option<String>,
    /// Domains to subscribe to (for projector/saga modes).
    pub domains: Option<Vec<String>>,
}

/// Target service configuration for sidecar modes.
#[derive(Debug, Clone, Deserialize)]
pub struct TargetConfig {
    /// gRPC address of the target service.
    pub address: String,
    /// Domain handled by this service (for aggregate mode).
    pub domain: Option<String>,
}

impl Config {
    /// Load configuration from file and environment.
    ///
    /// Priority (highest to lowest):
    /// 1. Environment variables
    /// 2. Config file
    /// 3. Defaults
    pub fn load() -> Result<Self, ConfigError> {
        // Try to load from file
        let config_path =
            std::env::var("ANGZARR_CONFIG").unwrap_or_else(|_| "config.yaml".to_string());

        let mut config = if Path::new(&config_path).exists() {
            Self::from_file(&config_path)?
        } else {
            Self::default()
        };

        // Override with environment variables
        config.apply_env_overrides();

        Ok(config)
    }

    /// Load configuration from a YAML file.
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::FileRead(path.to_string(), e.to_string()))?;

        serde_yaml::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Apply environment variable overrides.
    fn apply_env_overrides(&mut self) {
        // Mode override
        if let Ok(mode) = std::env::var("ANGZARR_MODE") {
            self.mode = match mode.to_lowercase().as_str() {
                "aggregate" => Mode::Aggregate,
                "projector" => Mode::Projector,
                "saga" => Mode::Saga,
                _ => Mode::Standalone,
            };
        }

        // Storage overrides
        if let Ok(storage_type) = std::env::var("STORAGE_TYPE") {
            self.storage.storage_type = storage_type;
        }

        if let Ok(path) = std::env::var("STORAGE_PATH") {
            self.storage.path = path;
        }

        if let Ok(database) = std::env::var("STORAGE_DATABASE") {
            self.storage.database = Some(database);
        }

        if let Ok(enabled) = std::env::var("STORAGE_SNAPSHOTS_ENABLE_READ") {
            self.storage.snapshots_enable.read = enabled.to_lowercase() == "true" || enabled == "1";
        }

        if let Ok(enabled) = std::env::var("STORAGE_SNAPSHOTS_ENABLE_WRITE") {
            self.storage.snapshots_enable.write =
                enabled.to_lowercase() == "true" || enabled == "1";
        }

        // Server overrides
        if let Ok(port) = std::env::var("COMMAND_HANDLER_PORT") {
            if let Ok(p) = port.parse() {
                self.server.command_handler_port = p;
            }
        }

        if let Ok(port) = std::env::var("EVENT_QUERY_PORT") {
            if let Ok(p) = port.parse() {
                self.server.event_query_port = p;
            }
        }

        if let Ok(host) = std::env::var("SERVER_HOST") {
            self.server.host = host;
        }

        // AMQP overrides
        if let Ok(url) = std::env::var("AMQP_URL") {
            if self.amqp.is_none() {
                self.amqp = Some(AmqpConfig {
                    url: url.clone(),
                    domain: None,
                    domains: None,
                });
            } else if let Some(ref mut amqp) = self.amqp {
                amqp.url = url;
            }
        }

        if let Ok(domain) = std::env::var("AMQP_DOMAIN") {
            if let Some(ref mut amqp) = self.amqp {
                amqp.domain = Some(domain);
            }
        }

        // Target overrides
        if let Ok(address) = std::env::var("TARGET_ADDRESS") {
            if self.target.is_none() {
                self.target = Some(TargetConfig {
                    address: address.clone(),
                    domain: None,
                });
            } else if let Some(ref mut target) = self.target {
                target.address = address;
            }
        }

        if let Ok(domain) = std::env::var("TARGET_DOMAIN") {
            if let Some(ref mut target) = self.target {
                target.domain = Some(domain);
            }
        }
    }

    /// Get business logic addresses as a HashMap.
    pub fn business_logic_addresses(&self) -> HashMap<String, String> {
        self.business_logic
            .iter()
            .map(|e| (e.domain.clone(), format!("http://{}", e.address)))
            .collect()
    }
}

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config file '{0}': {1}")]
    FileRead(String, String),

    #[error("Failed to parse config: {0}")]
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.storage.storage_type, "sqlite");
        assert_eq!(config.storage.path, "./data/events.db");
        assert_eq!(config.server.command_handler_port, 1313);
        assert_eq!(config.server.event_query_port, 1314);
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
storage:
  type: sqlite
  path: /tmp/test.db

server:
  command_handler_port: 8080
  event_query_port: 8081
  host: localhost

business_logic:
  - domain: orders
    address: localhost:50051
  - domain: inventory
    address: localhost:50052

projectors:
  - name: ui
    address: localhost:60001
    synchronous: true

sagas:
  - name: order_saga
    address: localhost:70001
"#;

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.storage.path, "/tmp/test.db");
        assert_eq!(config.server.command_handler_port, 8080);
        assert_eq!(config.business_logic.len(), 2);
        assert_eq!(config.business_logic[0].domain, "orders");
        assert_eq!(config.projectors.len(), 1);
        assert!(config.projectors[0].synchronous);
        assert_eq!(config.sagas.len(), 1);
    }

    #[test]
    fn test_from_file_valid() {
        let yaml = r#"
storage:
  type: sqlite
  path: /tmp/from_file.db
server:
  command_handler_port: 9000
"#;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let config = Config::from_file(file.path().to_str().unwrap()).unwrap();

        assert_eq!(config.storage.path, "/tmp/from_file.db");
        assert_eq!(config.server.command_handler_port, 9000);
    }

    #[test]
    fn test_from_file_not_found() {
        let result = Config::from_file("/nonexistent/path/config.yaml");

        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::FileRead(path, _) => {
                assert_eq!(path, "/nonexistent/path/config.yaml");
            }
            _ => panic!("Expected FileRead error"),
        }
    }

    #[test]
    fn test_from_file_invalid_yaml() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"not: valid: yaml: content: [[[").unwrap();

        let result = Config::from_file(file.path().to_str().unwrap());

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::Parse(_)));
    }

    #[test]
    fn test_business_logic_addresses() {
        let config = Config {
            business_logic: vec![
                BusinessLogicEndpoint {
                    domain: "orders".to_string(),
                    address: "localhost:50051".to_string(),
                },
                BusinessLogicEndpoint {
                    domain: "inventory".to_string(),
                    address: "localhost:50052".to_string(),
                },
            ],
            ..Default::default()
        };

        let addresses = config.business_logic_addresses();

        assert_eq!(addresses.len(), 2);
        assert_eq!(addresses.get("orders").unwrap(), "http://localhost:50051");
        assert_eq!(
            addresses.get("inventory").unwrap(),
            "http://localhost:50052"
        );
    }

    #[test]
    fn test_storage_config_default() {
        let storage = StorageConfig::default();
        assert_eq!(storage.storage_type, "sqlite");
        assert_eq!(storage.path, "./data/events.db");
        assert!(storage.snapshots_enable.read);
        assert!(storage.snapshots_enable.write);
    }

    #[test]
    fn test_snapshots_enable_config_default() {
        let config = SnapshotsEnableConfig::default();
        assert!(config.read);
        assert!(config.write);
    }

    #[test]
    fn test_apply_env_overrides_snapshots_read_disabled() {
        let mut config = Config::default();
        std::env::set_var("STORAGE_SNAPSHOTS_ENABLE_READ", "false");

        config.apply_env_overrides();

        assert!(!config.storage.snapshots_enable.read);
        std::env::remove_var("STORAGE_SNAPSHOTS_ENABLE_READ");
    }

    #[test]
    fn test_apply_env_overrides_snapshots_write_disabled() {
        let mut config = Config::default();
        std::env::set_var("STORAGE_SNAPSHOTS_ENABLE_WRITE", "false");

        config.apply_env_overrides();

        assert!(!config.storage.snapshots_enable.write);
        std::env::remove_var("STORAGE_SNAPSHOTS_ENABLE_WRITE");
    }

    #[test]
    fn test_apply_env_overrides_snapshots_read_enabled() {
        let mut config = Config::default();
        config.storage.snapshots_enable.read = false;
        std::env::set_var("STORAGE_SNAPSHOTS_ENABLE_READ", "true");

        config.apply_env_overrides();

        assert!(config.storage.snapshots_enable.read);
        std::env::remove_var("STORAGE_SNAPSHOTS_ENABLE_READ");
    }

    #[test]
    fn test_apply_env_overrides_snapshots_write_enabled() {
        let mut config = Config::default();
        config.storage.snapshots_enable.write = false;
        std::env::set_var("STORAGE_SNAPSHOTS_ENABLE_WRITE", "true");

        config.apply_env_overrides();

        assert!(config.storage.snapshots_enable.write);
        std::env::remove_var("STORAGE_SNAPSHOTS_ENABLE_WRITE");
    }

    #[test]
    fn test_server_config_default() {
        let server = ServerConfig::default();
        assert_eq!(server.command_handler_port, 1313);
        assert_eq!(server.event_query_port, 1314);
        assert_eq!(server.host, "0.0.0.0");
    }

    #[test]
    fn test_projector_endpoint_default() {
        let endpoint = ProjectorEndpoint::default();
        assert_eq!(endpoint.name, "");
        assert_eq!(endpoint.address, "");
        assert!(!endpoint.synchronous);
    }

    #[test]
    fn test_saga_endpoint_default() {
        let endpoint = SagaEndpoint::default();
        assert_eq!(endpoint.name, "");
        assert_eq!(endpoint.address, "");
        assert!(!endpoint.synchronous);
    }

    #[test]
    fn test_saga_compensation_config_default() {
        let config = SagaCompensationConfig::default();
        assert_eq!(config.fallback_domain, "angzarr.saga-failures");
        assert!(config.dead_letter_queue_url.is_none());
        assert!(config.escalation_webhook_url.is_none());
        assert!(config.fallback_emit_system_revocation);
        assert!(!config.fallback_send_to_dlq);
        assert!(!config.fallback_escalate);
    }

    #[test]
    fn test_apply_env_overrides_storage_path() {
        let mut config = Config::default();
        std::env::set_var("STORAGE_PATH", "/custom/path.db");

        config.apply_env_overrides();

        assert_eq!(config.storage.path, "/custom/path.db");
        std::env::remove_var("STORAGE_PATH");
    }

    #[test]
    fn test_apply_env_overrides_command_handler_port() {
        let mut config = Config::default();
        std::env::set_var("COMMAND_HANDLER_PORT", "9999");

        config.apply_env_overrides();

        assert_eq!(config.server.command_handler_port, 9999);
        std::env::remove_var("COMMAND_HANDLER_PORT");
    }

    #[test]
    fn test_apply_env_overrides_event_query_port() {
        let mut config = Config::default();
        std::env::set_var("EVENT_QUERY_PORT", "8888");

        config.apply_env_overrides();

        assert_eq!(config.server.event_query_port, 8888);
        std::env::remove_var("EVENT_QUERY_PORT");
    }

    #[test]
    fn test_apply_env_overrides_server_host() {
        let mut config = Config::default();
        std::env::set_var("SERVER_HOST", "127.0.0.1");

        config.apply_env_overrides();

        assert_eq!(config.server.host, "127.0.0.1");
        std::env::remove_var("SERVER_HOST");
    }

    #[test]
    fn test_apply_env_overrides_invalid_port_ignored() {
        let mut config = Config::default();
        let original_port = config.server.command_handler_port;
        std::env::set_var("COMMAND_HANDLER_PORT", "not_a_number");

        config.apply_env_overrides();

        assert_eq!(config.server.command_handler_port, original_port);
        std::env::remove_var("COMMAND_HANDLER_PORT");
    }
}
