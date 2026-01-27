//! Application configuration.
//!
//! Aggregates configuration from all modules into a single Config struct
//! that can be loaded from YAML files or environment variables.

mod server;

pub use server::{
    ConfigError, ExternalServiceConfig, GatewayConfig, HealthCheckConfig, ResolvedStandaloneConfig,
    ServerConfig, ServiceConfig, ServiceConfigRef, StandaloneConfig, TargetConfig,
};

use serde::Deserialize;

use crate::bus::MessagingConfig;
use crate::clients::{ProcessManagerConfig, SagaCompensationConfig, ServiceEndpoint};
use crate::services::UpcasterConfig;
use crate::storage::StorageConfig;
use crate::transport::TransportConfig;

/// Main application configuration.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Server configuration.
    pub server: ServerConfig,
    /// Storage configuration.
    pub storage: StorageConfig,
    /// Transport configuration.
    pub transport: TransportConfig,
    /// Messaging configuration (optional).
    pub messaging: Option<MessagingConfig>,
    /// Target service for sidecar mode.
    pub target: Option<TargetConfig>,
    /// Business logic endpoints (gateway mode).
    pub business_logic: Option<Vec<ServiceEndpoint>>,
    /// Projector endpoints.
    pub projectors: Option<Vec<ServiceEndpoint>>,
    /// Saga endpoints.
    pub sagas: Option<Vec<ServiceEndpoint>>,
    /// Process manager configurations.
    pub process_managers: Option<Vec<ProcessManagerConfig>>,
    /// Saga compensation configuration.
    pub saga_compensation: Option<SagaCompensationConfig>,
    /// Standalone mode configuration (services, gateway).
    pub standalone: StandaloneConfig,
    /// Upcaster configuration for event version transformation.
    pub upcaster: UpcasterConfig,
}

impl Config {
    /// Load configuration from file and environment.
    ///
    /// Configuration sources (in order of priority, later overrides earlier):
    /// 1. `config.yaml` in current directory (if exists)
    /// 2. File specified by `ANGZARR_CONFIG` environment variable (if set)
    /// 3. Environment variables with `ANGZARR_` prefix
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        use ::config::{Config as ConfigLib, Environment, File, FileFormat};

        let mut builder = ConfigLib::builder()
            // Start with defaults from config.yaml in current directory
            .add_source(File::new("config", FileFormat::Yaml).required(false))
            .add_source(File::new("config.yaml", FileFormat::Yaml).required(false));

        // Add config file from ANGZARR_CONFIG env var if set
        if let Ok(config_path) = std::env::var("ANGZARR_CONFIG") {
            builder = builder.add_source(File::new(&config_path, FileFormat::Yaml).required(true));
        }

        let config = builder
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

/// Get the base directory for resolving file references in configs.
///
/// Returns the parent directory of ANGZARR_CONFIG if set, otherwise current directory.
pub fn config_base_dir() -> std::path::PathBuf {
    if let Ok(config_path) = std::env::var("ANGZARR_CONFIG") {
        let path = std::path::Path::new(&config_path);
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    } else {
        std::path::PathBuf::from(".")
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
