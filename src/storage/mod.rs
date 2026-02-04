//! Event and snapshot storage.
//!
//! This module contains:
//! - `EventStore` trait: Event persistence
//! - `SnapshotStore` trait: Snapshot optimization
//! - `PositionStore` trait: Handler checkpoint tracking
//! - Storage configuration types
//! - Implementations: MongoDB, PostgreSQL, SQLite, Redis, Bigtable, DynamoDB

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
pub mod helpers;
pub mod mock;
#[cfg(feature = "bigtable")]
pub mod bigtable;
#[cfg(feature = "dynamo")]
pub mod dynamo;
#[cfg(feature = "mongodb")]
pub mod mongodb;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "redis")]
pub mod redis;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod schema;
#[cfg(feature = "sqlite")]
pub mod sqlite;

// Re-exports
pub use mock::{MockEventStore, MockPositionStore, MockSnapshotStore};
#[cfg(feature = "bigtable")]
pub use bigtable::{BigtableEventStore, BigtablePositionStore, BigtableSnapshotStore};
#[cfg(feature = "dynamo")]
pub use dynamo::{DynamoEventStore, DynamoPositionStore, DynamoSnapshotStore};
#[cfg(feature = "mongodb")]
pub use mongodb::{MongoEventStore, MongoPositionStore, MongoSnapshotStore};
#[cfg(all(feature = "mongodb", feature = "topology"))]
pub use mongodb::MongoTopologyStore;
#[cfg(feature = "postgres")]
pub use postgres::{PostgresEventStore, PostgresPositionStore, PostgresSnapshotStore};
#[cfg(all(feature = "postgres", feature = "topology"))]
pub use postgres::PostgresTopologyStore;
#[cfg(feature = "redis")]
pub use redis::{RedisEventStore, RedisPositionStore, RedisSnapshotStore};
#[cfg(all(feature = "redis", feature = "topology"))]
pub use redis::RedisTopologyStore;
#[cfg(feature = "sqlite")]
pub use sqlite::{SqliteEventStore, SqlitePositionStore, SqliteSnapshotStore};
#[cfg(all(feature = "sqlite", feature = "topology"))]
pub use sqlite::SqliteTopologyStore;

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

    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Protobuf decode error: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),

    #[error("Cover missing from EventBook")]
    MissingCover,

    #[error("Root UUID missing from Cover")]
    MissingRoot,

    #[cfg(feature = "mongodb")]
    #[error("MongoDB error: {0}")]
    Mongo(#[from] ::mongodb::error::Error),

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
    Mongodb,
    Postgres,
    Sqlite,
    Redis,
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
    /// SQLite-specific configuration.
    pub sqlite: SqliteConfig,
    /// Redis-specific configuration.
    pub redis: RedisConfig,
    /// Snapshot enable/disable flags for debugging and troubleshooting.
    pub snapshots_enable: SnapshotsEnableConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: StorageType::Mongodb,
            mongodb: MongodbConfig::default(),
            postgres: PostgresConfig::default(),
            sqlite: SqliteConfig::default(),
            redis: RedisConfig::default(),
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
/// - MongoDB: `--features mongodb` (included in default)
/// - PostgreSQL: `--features postgres`
/// - SQLite: `--features sqlite`
/// - Redis: `--features redis`
pub async fn init_storage(
    config: &StorageConfig,
) -> std::result::Result<(Arc<dyn EventStore>, Arc<dyn SnapshotStore>), Box<dyn std::error::Error>>
{
    match config.storage_type {
        StorageType::Mongodb => {
            #[cfg(feature = "mongodb")]
            {
                info!(
                    "Storage: mongodb at {} (db: {})",
                    config.mongodb.uri, config.mongodb.database
                );

                let client = ::mongodb::Client::with_uri_str(&config.mongodb.uri).await?;

                let event_store =
                    Arc::new(MongoEventStore::new(&client, &config.mongodb.database).await?);
                let snapshot_store =
                    Arc::new(MongoSnapshotStore::new(&client, &config.mongodb.database).await?);

                Ok((event_store, snapshot_store))
            }

            #[cfg(not(feature = "mongodb"))]
            {
                Err("MongoDB support requires the 'mongodb' feature. Rebuild with --features mongodb".into())
            }
        }
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
    }
}

/// Initialize a position store based on configuration.
///
/// Position stores track projector/saga checkpoints (last processed sequence).
/// Separate from `init_storage` because position tracking is per-handler,
/// not per-domain.
///
/// Requires the corresponding feature to be enabled:
/// - MongoDB: `--features mongodb`
/// - PostgreSQL: `--features postgres`
/// - SQLite: `--features sqlite`
pub async fn init_position_store(
    config: &StorageConfig,
) -> std::result::Result<Arc<dyn PositionStore>, Box<dyn std::error::Error>> {
    match config.storage_type {
        StorageType::Mongodb => {
            #[cfg(feature = "mongodb")]
            {
                info!(
                    "PositionStore: mongodb at {} (db: {})",
                    config.mongodb.uri, config.mongodb.database
                );
                let client = ::mongodb::Client::with_uri_str(&config.mongodb.uri).await?;
                Ok(Arc::new(
                    MongoPositionStore::new(&client, &config.mongodb.database).await?,
                ))
            }
            #[cfg(not(feature = "mongodb"))]
            {
                Err("MongoDB position store requires the 'mongodb' feature".into())
            }
        }
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
