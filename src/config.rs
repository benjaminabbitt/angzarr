//! Configuration for evented server.
//!
//! Supports YAML file and environment variable overrides.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Server configuration.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Storage configuration.
    pub storage: StorageConfig,
    /// Server port configuration.
    pub server: ServerConfig,
    /// Business logic service endpoints.
    pub business_logic: Vec<BusinessLogicEndpoint>,
    /// Projector endpoints.
    pub projectors: Vec<ProjectorEndpoint>,
    /// Saga endpoints.
    pub sagas: Vec<SagaEndpoint>,
}

/// Storage configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Storage type (sqlite).
    #[serde(rename = "type")]
    pub storage_type: String,
    /// Path to database file.
    pub path: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: "sqlite".to_string(),
            path: "./data/events.db".to_string(),
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
            std::env::var("EVENTED_CONFIG").unwrap_or_else(|_| "config.yaml".to_string());

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
        if let Ok(path) = std::env::var("STORAGE_PATH") {
            self.storage.path = path;
        }

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
}
