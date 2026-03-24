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
#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct PostgresConfig {
    /// PostgreSQL connection URI.
    pub uri: String,
}

impl std::fmt::Debug for PostgresConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresConfig")
            .field("uri", &"<redacted>")
            .finish()
    }
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
#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct RedisConfig {
    /// Redis connection URI.
    pub uri: String,
}

impl std::fmt::Debug for RedisConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisConfig")
            .field("uri", &"<redacted>")
            .finish()
    }
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
    /// Query timeout in milliseconds for consuming messages from streams.
    /// Default: 100ms.
    pub query_timeout_ms: u64,
}

#[cfg(feature = "nats")]
impl Default for NatsStorageConfig {
    fn default() -> Self {
        Self {
            url: "nats://localhost:4222".to_string(),
            bucket_prefix: "angzarr".to_string(),
            query_timeout_ms: 100,
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
#[path = "config.test.rs"]
mod tests;
