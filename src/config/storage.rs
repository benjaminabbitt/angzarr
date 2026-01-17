//! Storage configuration types.

use serde::Deserialize;

/// Storage type discriminator.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    #[default]
    Mongodb,
    Postgres,
    Eventstoredb,
}

/// Storage configuration (discriminated union).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Storage type discriminator.
    #[serde(rename = "type")]
    pub storage_type: StorageType,
    /// MongoDB-specific configuration.
    pub mongodb: MongodbConfig,
    /// PostgreSQL-specific configuration.
    pub postgres: PostgresConfig,
    /// EventStoreDB-specific configuration.
    pub eventstoredb: EventStoreDbConfig,
    /// Snapshot enable/disable flags for debugging and troubleshooting.
    pub snapshots_enable: SnapshotsEnableConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: StorageType::Mongodb,
            mongodb: MongodbConfig::default(),
            postgres: PostgresConfig::default(),
            eventstoredb: EventStoreDbConfig::default(),
            snapshots_enable: SnapshotsEnableConfig::default(),
        }
    }
}

/// MongoDB-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MongodbConfig {
    /// MongoDB connection URI.
    pub uri: String,
    /// Database name.
    pub database: String,
}

impl Default for MongodbConfig {
    fn default() -> Self {
        Self {
            uri: "mongodb://localhost:27017".to_string(),
            database: "angzarr".to_string(),
        }
    }
}

/// PostgreSQL-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PostgresConfig {
    /// PostgreSQL connection URI.
    pub uri: String,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            uri: "postgres://localhost:5432/angzarr".to_string(),
        }
    }
}

/// EventStoreDB-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct EventStoreDbConfig {
    /// EventStoreDB connection string.
    pub connection_string: String,
}

impl Default for EventStoreDbConfig {
    fn default() -> Self {
        Self {
            connection_string: "esdb://localhost:2113?tls=false".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_config_default() {
        let storage = StorageConfig::default();
        assert_eq!(storage.storage_type, StorageType::Mongodb);
        assert_eq!(storage.mongodb.uri, "mongodb://localhost:27017");
        assert_eq!(storage.mongodb.database, "angzarr");
        assert!(storage.snapshots_enable.read);
        assert!(storage.snapshots_enable.write);
    }

    #[test]
    fn test_snapshots_enable_config_default() {
        let config = SnapshotsEnableConfig::default();
        assert!(config.read);
        assert!(config.write);
    }
}
