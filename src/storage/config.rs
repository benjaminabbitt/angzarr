//! Storage configuration types.

use serde::Deserialize;

// ============================================================================
// Configuration
// ============================================================================

/// Storage configuration (discriminated union).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Storage type discriminator (e.g., "postgres", "sqlite", "bigtable", "dynamo", "nats").
    #[serde(rename = "type")]
    pub storage_type: String,
    /// PostgreSQL-specific configuration.
    pub postgres: PostgresConfig,
    /// SQLite-specific configuration.
    pub sqlite: SqliteConfig,
    /// Redis-specific configuration.
    pub redis: RedisConfig,
    /// Bigtable-specific configuration.
    #[cfg(feature = "bigtable")]
    pub bigtable: super::bigtable::BigtableConfig,
    /// DynamoDB-specific configuration.
    #[cfg(feature = "dynamo")]
    pub dynamo: super::dynamo::DynamoConfig,
    /// NATS-specific configuration.
    #[cfg(feature = "nats")]
    pub nats: NatsStorageConfig,
    /// Snapshot enable/disable flags for debugging and troubleshooting.
    pub snapshots_enable: SnapshotsEnableConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: "postgres".to_string(),
            postgres: PostgresConfig::default(),
            sqlite: SqliteConfig::default(),
            redis: RedisConfig::default(),
            #[cfg(feature = "bigtable")]
            bigtable: super::bigtable::BigtableConfig::default(),
            #[cfg(feature = "dynamo")]
            dynamo: super::dynamo::DynamoConfig::default(),
            #[cfg(feature = "nats")]
            nats: NatsStorageConfig::default(),
            snapshots_enable: SnapshotsEnableConfig::default(),
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

/// SQLite-specific configuration.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SqliteConfig {
    /// SQLite database path.
    /// If empty or not set, uses in-memory database (:memory:).
    pub path: Option<String>,
}

impl SqliteConfig {
    /// Get the connection URI for SQLite.
    /// Returns in-memory URI if path is not configured.
    pub fn uri(&self) -> String {
        match &self.path {
            Some(path) if !path.is_empty() => format!("sqlite:{}", path),
            _ => "sqlite::memory:".to_string(),
        }
    }
}

/// Redis-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RedisConfig {
    /// Redis connection URI.
    pub uri: String,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            uri: "redis://localhost:6379".to_string(),
        }
    }
}

/// NATS storage configuration.
#[cfg(feature = "nats")]
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NatsStorageConfig {
    /// NATS server URL.
    pub url: String,
    /// Bucket prefix for KV stores.
    pub bucket_prefix: String,
}

#[cfg(feature = "nats")]
impl Default for NatsStorageConfig {
    fn default() -> Self {
        Self {
            url: "nats://localhost:4222".to_string(),
            bucket_prefix: "angzarr".to_string(),
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
    //! Tests for storage configuration types.
    //!
    //! Storage config is a discriminated union supporting multiple backends:
    //! PostgreSQL (default), SQLite, Redis, Bigtable, DynamoDB, NATS.
    //! Each backend has its own sub-configuration.
    //!
    //! Key behaviors verified:
    //! - Default storage type is postgres (production-ready default)
    //! - SQLite URI handling (in-memory vs file paths)
    //! - Snapshot enable flags for debugging/troubleshooting

    use super::*;

    // ============================================================================
    // StorageConfig Tests
    // ============================================================================

    /// Default storage config targets PostgreSQL with standard connection string.
    ///
    /// PostgreSQL is the default because it's the most commonly deployed
    /// production database with full ACID guarantees.
    #[test]
    fn test_storage_config_default() {
        let storage = StorageConfig::default();
        assert_eq!(storage.storage_type, "postgres");
        assert_eq!(storage.postgres.uri, "postgres://localhost:5432/angzarr");
        assert!(storage.snapshots_enable.read);
        assert!(storage.snapshots_enable.write);
    }

    // ============================================================================
    // SqliteConfig Tests
    // ============================================================================

    /// Default SQLite config uses in-memory database.
    ///
    /// In-memory is safest for testing—no file cleanup needed.
    /// Production deployments should configure explicit path.
    #[test]
    fn test_sqlite_uri_memory() {
        let config = SqliteConfig::default();
        assert_eq!(config.uri(), "sqlite::memory:");
    }

    /// File path SQLite config generates correct URI.
    #[test]
    fn test_sqlite_uri_file() {
        let config = SqliteConfig {
            path: Some("/tmp/test.db".to_string()),
        };
        assert_eq!(config.uri(), "sqlite:/tmp/test.db");
    }

    /// Empty path string treated as in-memory (not empty file path).
    ///
    /// Edge case: config deserialization may produce `Some("")` rather than
    /// `None`. The uri() method treats this as in-memory to avoid creating
    /// a database at the current directory with no name.
    #[test]
    fn test_sqlite_uri_empty_string_is_memory() {
        let config = SqliteConfig {
            path: Some(String::new()),
        };
        assert_eq!(config.uri(), "sqlite::memory:");
    }

    // ============================================================================
    // SnapshotsEnableConfig Tests
    // ============================================================================

    /// Default snapshot config enables both read and write.
    ///
    /// Snapshots improve performance by avoiding full event replay.
    /// Both flags are enabled by default; disable for debugging:
    /// - read=false: Force full event replay (verify replay correctness)
    /// - write=false: Pure event sourcing mode (no snapshot storage)
    #[test]
    fn test_snapshots_enable_config_default() {
        let config = SnapshotsEnableConfig::default();
        assert!(config.read);
        assert!(config.write);
    }
}
