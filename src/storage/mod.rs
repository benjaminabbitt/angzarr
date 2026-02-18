//! Event and snapshot storage.
//!
//! This module contains:
//! - `EventStore` trait: Event persistence
//! - `SnapshotStore` trait: Snapshot optimization
//! - `PositionStore` trait: Handler checkpoint tracking
//! - Storage configuration types
//! - Implementations: MongoDB, PostgreSQL, SQLite, Redis, Bigtable, DynamoDB, ImmuDB

use std::sync::Arc;

use serde::Deserialize;
use tracing::info;
use uuid::Uuid;

// Trait modules
mod event_store;
mod position_store;
mod snapshot_store;

pub use event_store::EventStore;
pub use position_store::PositionStore;
pub use snapshot_store::SnapshotStore;

// Implementation modules
#[cfg(feature = "bigtable")]
pub mod bigtable;
#[cfg(feature = "dynamo")]
pub mod dynamo;
pub mod helpers;
#[cfg(feature = "immudb")]
pub mod immudb;
pub mod mock;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "redis")]
pub mod redis;
#[cfg(any(feature = "postgres", feature = "sqlite", feature = "immudb"))]
pub mod schema;
#[cfg(feature = "sqlite")]
pub mod sqlite;

// Re-exports
#[cfg(feature = "bigtable")]
pub use bigtable::{
    BigtableConfig, BigtableEventStore, BigtablePositionStore, BigtableSnapshotStore,
};
#[cfg(feature = "dynamo")]
pub use dynamo::{DynamoConfig, DynamoEventStore, DynamoPositionStore, DynamoSnapshotStore};
#[cfg(feature = "immudb")]
pub use immudb::ImmudbEventStore;
pub use mock::{MockEventStore, MockPositionStore, MockSnapshotStore};
#[cfg(all(feature = "postgres", feature = "topology"))]
pub use postgres::PostgresTopologyStore;
#[cfg(feature = "postgres")]
pub use postgres::{PostgresEventStore, PostgresPositionStore, PostgresSnapshotStore};
#[cfg(all(feature = "redis", feature = "topology"))]
pub use redis::RedisTopologyStore;
#[cfg(feature = "redis")]
pub use redis::{RedisEventStore, RedisPositionStore, RedisSnapshotStore};
#[cfg(all(feature = "sqlite", feature = "topology"))]
pub use sqlite::SqliteTopologyStore;
#[cfg(feature = "sqlite")]
pub use sqlite::{SqliteEventStore, SqlitePositionStore, SqliteSnapshotStore};

// ============================================================================
// Error Types
// ============================================================================

/// Result type for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;

/// Errors that can occur during storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Event not found: domain={domain}, root={root}")]
    NotFound { domain: String, root: Uuid },

    #[error("Sequence conflict: expected {expected}, got {actual}")]
    SequenceConflict { expected: u32, actual: u32 },

    #[error("Invalid timestamp: seconds={seconds}, nanos={nanos}")]
    InvalidTimestamp { seconds: i64, nanos: i32 },

    #[error("Invalid timestamp format: {0}")]
    InvalidTimestampFormat(String),

    #[error("Invalid divergence point: {0}")]
    InvalidDivergencePoint(String),

    #[error("Invalid UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),

    #[cfg(any(feature = "postgres", feature = "sqlite", feature = "immudb"))]
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Protobuf decode error: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),

    #[error("Cover missing from EventBook")]
    MissingCover,

    #[error("Root UUID missing from Cover")]
    MissingRoot,

    #[cfg(feature = "redis")]
    #[error("Redis error: {0}")]
    Redis(#[from] ::redis::RedisError),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

// ============================================================================
// Configuration
// ============================================================================

/// Storage type discriminator.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    #[default]
    Postgres,
    Sqlite,
    Redis,
    Bigtable,
    Dynamo,
}

/// Storage configuration (discriminated union).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Storage type discriminator.
    #[serde(rename = "type")]
    pub storage_type: StorageType,
    /// PostgreSQL-specific configuration.
    pub postgres: PostgresConfig,
    /// SQLite-specific configuration.
    pub sqlite: SqliteConfig,
    /// Redis-specific configuration.
    pub redis: RedisConfig,
    /// Bigtable-specific configuration.
    #[cfg(feature = "bigtable")]
    pub bigtable: bigtable::BigtableConfig,
    /// DynamoDB-specific configuration.
    #[cfg(feature = "dynamo")]
    pub dynamo: dynamo::DynamoConfig,
    /// Snapshot enable/disable flags for debugging and troubleshooting.
    pub snapshots_enable: SnapshotsEnableConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: StorageType::Postgres,
            postgres: PostgresConfig::default(),
            sqlite: SqliteConfig::default(),
            redis: RedisConfig::default(),
            #[cfg(feature = "bigtable")]
            bigtable: bigtable::BigtableConfig::default(),
            #[cfg(feature = "dynamo")]
            dynamo: dynamo::DynamoConfig::default(),
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

// ============================================================================
// Factory
// ============================================================================

/// Initialize storage based on configuration.
///
/// Returns tuple of (EventStore, SnapshotStore) implementations based on
/// the configured storage type.
///
/// Requires the corresponding feature to be enabled:
/// - PostgreSQL: `--features postgres` (included in default)
/// - SQLite: `--features sqlite`
/// - Redis: `--features redis`
pub async fn init_storage(
    config: &StorageConfig,
) -> std::result::Result<(Arc<dyn EventStore>, Arc<dyn SnapshotStore>), Box<dyn std::error::Error>>
{
    match config.storage_type {
        StorageType::Postgres => {
            #[cfg(feature = "postgres")]
            {
                info!("Storage: postgres at {}", config.postgres.uri);

                let pool = sqlx::PgPool::connect(&config.postgres.uri).await?;
                sqlx::migrate!("migrations/postgres").run(&pool).await?;

                let event_store = Arc::new(PostgresEventStore::new(pool.clone()));
                let snapshot_store = Arc::new(PostgresSnapshotStore::new(pool));

                Ok((event_store, snapshot_store))
            }

            #[cfg(not(feature = "postgres"))]
            {
                Err("PostgreSQL support requires the 'postgres' feature. Rebuild with --features postgres".into())
            }
        }
        StorageType::Sqlite => {
            #[cfg(feature = "sqlite")]
            {
                use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
                use std::str::FromStr;
                use std::time::Duration;

                let uri = config.sqlite.uri();
                info!("Storage: sqlite at {}", uri);

                // Configure SQLite for concurrent access:
                // - WAL mode: allows concurrent readers during writes
                // - busy_timeout: wait instead of failing on lock contention
                // - create_if_missing: create database file if it doesn't exist
                let connect_options = SqliteConnectOptions::from_str(&uri)?
                    .journal_mode(SqliteJournalMode::Wal)
                    .busy_timeout(Duration::from_secs(30))
                    .create_if_missing(true);

                let pool = SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect_with(connect_options)
                    .await?;

                sqlx::migrate!("migrations/sqlite").run(&pool).await?;

                let event_store = Arc::new(SqliteEventStore::new(pool.clone()));
                let snapshot_store = Arc::new(SqliteSnapshotStore::new(pool));

                Ok((event_store, snapshot_store))
            }

            #[cfg(not(feature = "sqlite"))]
            {
                Err(
                    "SQLite support requires the 'sqlite' feature. Rebuild with --features sqlite"
                        .into(),
                )
            }
        }
        StorageType::Redis => {
            #[cfg(feature = "redis")]
            {
                info!("Storage: redis at {}", config.redis.uri);

                let event_store =
                    Arc::new(redis::RedisEventStore::new(&config.redis.uri, None).await?);
                let snapshot_store =
                    Arc::new(redis::RedisSnapshotStore::new(&config.redis.uri, None).await?);

                Ok((event_store, snapshot_store))
            }

            #[cfg(not(feature = "redis"))]
            {
                Err(
                    "Redis support requires the 'redis' feature. Rebuild with --features redis"
                        .into(),
                )
            }
        }
        StorageType::Bigtable => {
            #[cfg(feature = "bigtable")]
            {
                info!(
                    "Storage: bigtable project={} instance={}",
                    config.bigtable.project_id, config.bigtable.instance_id
                );

                let emulator_host = config.bigtable.emulator_host.as_deref();

                let event_store = Arc::new(
                    BigtableEventStore::new(
                        &config.bigtable.project_id,
                        &config.bigtable.instance_id,
                        &config.bigtable.events_table,
                        emulator_host,
                    )
                    .await?,
                );

                let snapshot_store = Arc::new(
                    BigtableSnapshotStore::new(
                        &config.bigtable.project_id,
                        &config.bigtable.instance_id,
                        &config.bigtable.snapshots_table,
                        emulator_host,
                    )
                    .await?,
                );

                Ok((event_store, snapshot_store))
            }

            #[cfg(not(feature = "bigtable"))]
            {
                Err(
                    "Bigtable support requires the 'bigtable' feature. Rebuild with --features bigtable"
                        .into(),
                )
            }
        }
        StorageType::Dynamo => {
            #[cfg(feature = "dynamo")]
            {
                info!(
                    "Storage: dynamodb tables={}/{}/{}",
                    config.dynamo.events_table,
                    config.dynamo.snapshots_table,
                    config.dynamo.positions_table
                );

                let event_store = Arc::new(
                    DynamoEventStore::new(
                        &config.dynamo.events_table,
                        config.dynamo.endpoint_url.as_deref(),
                    )
                    .await?,
                );

                let snapshot_store = Arc::new(
                    DynamoSnapshotStore::new(
                        &config.dynamo.snapshots_table,
                        config.dynamo.endpoint_url.as_deref(),
                    )
                    .await?,
                );

                Ok((event_store, snapshot_store))
            }

            #[cfg(not(feature = "dynamo"))]
            {
                Err(
                    "DynamoDB support requires the 'dynamo' feature. Rebuild with --features dynamo"
                        .into(),
                )
            }
        }
    }
}

/// Initialize a position store based on configuration.
///
/// Position stores track projector/saga checkpoints (last processed sequence).
/// Separate from `init_storage` because position tracking is per-handler,
/// not per-domain.
///
/// Requires the corresponding feature to be enabled:
/// - PostgreSQL: `--features postgres` (included in default)
/// - SQLite: `--features sqlite`
/// - Redis: `--features redis`
pub async fn init_position_store(
    config: &StorageConfig,
) -> std::result::Result<Arc<dyn PositionStore>, Box<dyn std::error::Error>> {
    match config.storage_type {
        StorageType::Postgres => {
            #[cfg(feature = "postgres")]
            {
                info!("PositionStore: postgres at {}", config.postgres.uri);
                let pool = sqlx::PgPool::connect(&config.postgres.uri).await?;
                sqlx::migrate!("migrations/postgres").run(&pool).await?;
                Ok(Arc::new(PostgresPositionStore::new(pool)))
            }
            #[cfg(not(feature = "postgres"))]
            {
                Err("PostgreSQL position store requires the 'postgres' feature".into())
            }
        }
        StorageType::Sqlite => {
            #[cfg(feature = "sqlite")]
            {
                use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
                use std::str::FromStr;
                use std::time::Duration;

                let uri = config.sqlite.uri();
                info!("PositionStore: sqlite at {}", uri);

                let connect_options = SqliteConnectOptions::from_str(&uri)?
                    .journal_mode(SqliteJournalMode::Wal)
                    .busy_timeout(Duration::from_secs(30))
                    .create_if_missing(true);

                let pool = SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect_with(connect_options)
                    .await?;

                sqlx::migrate!("migrations/sqlite").run(&pool).await?;
                Ok(Arc::new(SqlitePositionStore::new(pool)))
            }
            #[cfg(not(feature = "sqlite"))]
            {
                Err("SQLite position store requires the 'sqlite' feature".into())
            }
        }
        StorageType::Redis => {
            #[cfg(feature = "redis")]
            {
                info!("PositionStore: redis at {}", config.redis.uri);
                Ok(Arc::new(
                    RedisPositionStore::new(&config.redis.uri, None).await?,
                ))
            }
            #[cfg(not(feature = "redis"))]
            {
                Err("Redis position store requires the 'redis' feature".into())
            }
        }
        StorageType::Bigtable => {
            #[cfg(feature = "bigtable")]
            {
                info!(
                    "PositionStore: bigtable project={} instance={}",
                    config.bigtable.project_id, config.bigtable.instance_id
                );
                Ok(Arc::new(
                    BigtablePositionStore::new(
                        &config.bigtable.project_id,
                        &config.bigtable.instance_id,
                        &config.bigtable.positions_table,
                        config.bigtable.emulator_host.as_deref(),
                    )
                    .await?,
                ))
            }
            #[cfg(not(feature = "bigtable"))]
            {
                Err("Bigtable position store requires the 'bigtable' feature".into())
            }
        }
        StorageType::Dynamo => {
            #[cfg(feature = "dynamo")]
            {
                info!(
                    "PositionStore: dynamodb table={}",
                    config.dynamo.positions_table
                );
                Ok(Arc::new(
                    DynamoPositionStore::new(
                        &config.dynamo.positions_table,
                        config.dynamo.endpoint_url.as_deref(),
                    )
                    .await?,
                ))
            }
            #[cfg(not(feature = "dynamo"))]
            {
                Err("DynamoDB position store requires the 'dynamo' feature".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_config_default() {
        let storage = StorageConfig::default();
        assert_eq!(storage.storage_type, StorageType::Postgres);
        assert_eq!(storage.postgres.uri, "postgres://localhost:5432/angzarr");
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
