//! Configuration for angzarr server.
//!
//! Supports YAML file and environment variable overrides.

mod business;
mod messaging;
mod server;
mod storage;

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

// Re-export all public types for backwards compatibility
pub use business::{
    BusinessLogicEndpoint, ProjectorEndpoint, SagaCompensationConfig, SagaEndpoint,
};
pub use messaging::{AmqpConfig, MessagingConfig, MessagingType};
pub use server::{ServerConfig, TargetConfig};
pub use storage::{
    EventStoreDbConfig, MongodbConfig, PostgresConfig, SnapshotsEnableConfig, StorageConfig,
    StorageType,
};

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
    /// Messaging configuration (for event bus).
    pub messaging: Option<MessagingConfig>,
    /// Target service address (for sidecar modes).
    pub target: Option<TargetConfig>,
    /// Business logic service endpoints (standalone mode).
    pub business_logic: Vec<BusinessLogicEndpoint>,
    /// Projector endpoints (standalone mode).
    pub projectors: Vec<ProjectorEndpoint>,
    /// Saga endpoints (standalone mode).
    pub sagas: Vec<SagaEndpoint>,
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

        // Storage type override
        if let Ok(storage_type) = std::env::var("STORAGE_TYPE") {
            self.storage.storage_type = match storage_type.to_lowercase().as_str() {
                "postgres" => StorageType::Postgres,
                "eventstoredb" => StorageType::Eventstoredb,
                _ => StorageType::Mongodb,
            };
        }

        // MongoDB config overrides
        if let Ok(uri) = std::env::var("STORAGE_MONGODB_URI") {
            self.storage.mongodb.uri = uri;
        }

        if let Ok(database) = std::env::var("STORAGE_MONGODB_DATABASE") {
            self.storage.mongodb.database = database;
        }

        if let Ok(enabled) = std::env::var("STORAGE_SNAPSHOTS_ENABLE_READ") {
            self.storage.snapshots_enable.read = enabled.to_lowercase() == "true" || enabled == "1";
        }

        if let Ok(enabled) = std::env::var("STORAGE_SNAPSHOTS_ENABLE_WRITE") {
            self.storage.snapshots_enable.write =
                enabled.to_lowercase() == "true" || enabled == "1";
        }

        // Server overrides
        if let Ok(port) = std::env::var("ENTITY_PORT") {
            if let Ok(p) = port.parse() {
                self.server.entity_port = p;
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

        // Messaging type override
        if let Ok(messaging_type) = std::env::var("MESSAGING_TYPE") {
            let msg_type = match messaging_type.to_lowercase().as_str() {
                "amqp" => MessagingType::Amqp,
                _ => MessagingType::Direct,
            };
            if let Some(ref mut messaging) = self.messaging {
                messaging.messaging_type = msg_type;
            } else {
                self.messaging = Some(MessagingConfig {
                    messaging_type: msg_type,
                    amqp: AmqpConfig::default(),
                });
            }
        }

        // AMQP config overrides
        if let Ok(url) = std::env::var("MESSAGING_AMQP_URL") {
            if let Some(ref mut messaging) = self.messaging {
                messaging.amqp.url = url;
            } else {
                self.messaging = Some(MessagingConfig {
                    messaging_type: MessagingType::Amqp,
                    amqp: AmqpConfig {
                        url,
                        ..Default::default()
                    },
                });
            }
        }

        if let Ok(domain) = std::env::var("MESSAGING_AMQP_DOMAIN") {
            if let Some(ref mut messaging) = self.messaging {
                messaging.amqp.domain = Some(domain);
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
        assert_eq!(config.storage.storage_type, StorageType::Mongodb);
        assert_eq!(config.storage.mongodb.uri, "mongodb://localhost:27017");
        assert_eq!(config.server.entity_port, 1313);
        assert_eq!(config.server.event_query_port, 1314);
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
storage:
  type: mongodb
  mongodb:
    uri: mongodb://custom:27017
    database: testdb

server:
  entity_port: 8080
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
        assert_eq!(config.storage.mongodb.uri, "mongodb://custom:27017");
        assert_eq!(config.storage.mongodb.database, "testdb");
        assert_eq!(config.server.entity_port, 8080);
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
  type: mongodb
  mongodb:
    uri: mongodb://file_test:27017
    database: from_file_db
server:
  entity_port: 9000
"#;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let config = Config::from_file(file.path().to_str().unwrap()).unwrap();

        assert_eq!(config.storage.mongodb.uri, "mongodb://file_test:27017");
        assert_eq!(config.storage.mongodb.database, "from_file_db");
        assert_eq!(config.server.entity_port, 9000);
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
    fn test_apply_env_overrides_storage_mongodb() {
        let mut config = Config::default();
        std::env::set_var("STORAGE_TYPE", "mongodb");
        std::env::set_var("STORAGE_MONGODB_URI", "mongodb://myhost:27017");
        std::env::set_var("STORAGE_MONGODB_DATABASE", "mydb");

        config.apply_env_overrides();

        assert_eq!(config.storage.storage_type, StorageType::Mongodb);
        assert_eq!(config.storage.mongodb.uri, "mongodb://myhost:27017");
        assert_eq!(config.storage.mongodb.database, "mydb");
        std::env::remove_var("STORAGE_TYPE");
        std::env::remove_var("STORAGE_MONGODB_URI");
        std::env::remove_var("STORAGE_MONGODB_DATABASE");
    }

    #[test]
    fn test_apply_env_overrides_entity_port() {
        let mut config = Config::default();
        std::env::set_var("ENTITY_PORT", "9999");

        config.apply_env_overrides();

        assert_eq!(config.server.entity_port, 9999);
        std::env::remove_var("ENTITY_PORT");
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
        let original_port = config.server.entity_port;
        std::env::set_var("ENTITY_PORT", "not_a_number");

        config.apply_env_overrides();

        assert_eq!(config.server.entity_port, original_port);
        std::env::remove_var("ENTITY_PORT");
    }
}
