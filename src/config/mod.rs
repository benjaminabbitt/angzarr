//! Application configuration.
//!
//! Aggregates configuration from all modules into a single Config struct
//! that can be loaded from YAML files or environment variables.

mod server;

pub use server::{ServerConfig, TargetConfig};

use serde::Deserialize;

use crate::bus::MessagingConfig;
use crate::clients::{BusinessLogicEndpoint, ProjectorEndpoint, SagaCompensationConfig, SagaEndpoint};
use crate::storage::StorageConfig;

/// Main application configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Server configuration.
    pub server: ServerConfig,
    /// Storage configuration.
    pub storage: StorageConfig,
    /// Messaging configuration (optional).
    pub messaging: Option<MessagingConfig>,
    /// Target service for sidecar mode.
    pub target: Option<TargetConfig>,
    /// Business logic endpoints (gateway mode).
    pub business_logic: Option<Vec<BusinessLogicEndpoint>>,
    /// Projector endpoints.
    pub projectors: Option<Vec<ProjectorEndpoint>>,
    /// Saga endpoints.
    pub sagas: Option<Vec<SagaEndpoint>>,
    /// Saga compensation configuration.
    pub saga_compensation: Option<SagaCompensationConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            storage: StorageConfig::default(),
            messaging: None,
            target: None,
            business_logic: None,
            projectors: None,
            sagas: None,
            saga_compensation: None,
        }
    }
}

impl Config {
    /// Load configuration from file and environment.
    ///
    /// Looks for config.yaml in the current directory, then checks
    /// environment variables with ANGZARR_ prefix.
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        use ::config::{Config as ConfigLib, Environment, File, FileFormat};

        let config = ConfigLib::builder()
            // Start with defaults
            .add_source(File::new("config", FileFormat::Yaml).required(false))
            .add_source(File::new("config.yaml", FileFormat::Yaml).required(false))
            // Environment variables with ANGZARR_ prefix
            .add_source(
                Environment::with_prefix("ANGZARR")
                    .separator("__")
                    .try_parsing(true),
            )
            // Legacy env vars for backwards compatibility
            .add_source(Environment::default().try_parsing(true))
            .build()?;

        let config: Config = config.try_deserialize()?;
        Ok(config)
    }

    /// Create config for testing.
    pub fn for_test() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.server.aggregate_port, 1313);
        assert!(config.messaging.is_none());
        assert!(config.target.is_none());
    }

    #[test]
    fn test_config_for_test() {
        let config = Config::for_test();
        assert_eq!(config.server.host, "0.0.0.0");
    }
}
