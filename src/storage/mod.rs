//! Event and snapshot storage.
//!
//! This module contains:
//! - `EventStore` trait: Event persistence
//! - `SnapshotStore` trait: Snapshot optimization
//! - Storage configuration types
//! - Implementations: MongoDB, PostgreSQL, Redis

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use tracing::info;
use uuid::Uuid;

use crate::proto::{EventPage, Snapshot};

// Implementation modules
pub mod mock;
#[cfg(feature = "mongodb")]
pub mod mongodb;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "redis")]
pub mod redis;
#[cfg(feature = "postgres")]
pub mod schema;

// Re-exports
pub use mock::{MockEventStore, MockSnapshotStore};
#[cfg(feature = "mongodb")]
pub use mongodb::{MongoEventStore, MongoSnapshotStore};
#[cfg(feature = "postgres")]
pub use postgres::{PostgresEventStore, PostgresSnapshotStore};

// ============================================================================
// Traits
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

    #[error("Invalid UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),

    #[cfg(feature = "postgres")]
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
}

/// Interface for event persistence.
///
/// Implementations:
/// - `MongoEventStore`: MongoDB storage
/// - `PostgresEventStore`: PostgreSQL storage
/// - `RedisEventStore`: Redis storage
/// - `MockEventStore`: In-memory mock for testing
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Store events for an aggregate root.
    ///
    /// Events are appended to the existing event stream for this root.
    /// Sequence numbers are validated for consistency.
    async fn add(&self, domain: &str, root: Uuid, events: Vec<EventPage>) -> Result<()>;

    /// Retrieve all events for an aggregate.
    async fn get(&self, domain: &str, root: Uuid) -> Result<Vec<EventPage>>;

    /// Retrieve events from sequence N onwards.
    async fn get_from(&self, domain: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>>;

    /// Retrieve events in range [from, to).
    async fn get_from_to(
        &self,
        domain: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>>;

    /// List all aggregate roots in a domain.
    async fn list_roots(&self, domain: &str) -> Result<Vec<Uuid>>;

    /// List all domains that have events stored.
    async fn list_domains(&self) -> Result<Vec<String>>;

    /// Get the next sequence number for an aggregate.
    async fn get_next_sequence(&self, domain: &str, root: Uuid) -> Result<u32>;
}

/// Interface for snapshot persistence.
///
/// Snapshots are optional optimization to avoid replaying entire event history.
/// When loading an aggregate, if a snapshot exists, events are loaded from
/// the snapshot sequence onwards.
///
/// # Requirements
///
/// For snapshotting to work, aggregate state must be protobuf serializable.
/// The state is stored as `google.protobuf.Any`, requiring:
/// - State type must be a protobuf `Message`
/// - State must implement `prost::Name` for type URL resolution
///
/// # Implementations
///
/// - `MongoSnapshotStore`: MongoDB storage
/// - `PostgresSnapshotStore`: PostgreSQL storage
/// - `RedisSnapshotStore`: Redis storage
/// - `MockSnapshotStore`: In-memory mock for testing
#[async_trait]
pub trait SnapshotStore: Send + Sync {
    /// Retrieve the latest snapshot for an aggregate.
    ///
    /// Returns `None` if no snapshot exists.
    async fn get(&self, domain: &str, root: Uuid) -> Result<Option<Snapshot>>;

    /// Store a snapshot for an aggregate.
    ///
    /// This replaces any existing snapshot for this root.
    async fn put(&self, domain: &str, root: Uuid, snapshot: Snapshot) -> Result<()>;

    /// Delete the snapshot for an aggregate.
    async fn delete(&self, domain: &str, root: Uuid) -> Result<()>;
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

                let event_store = Arc::new(PostgresEventStore::new(pool.clone()));
                event_store.init().await?;

                let snapshot_store = Arc::new(PostgresSnapshotStore::new(pool));
                snapshot_store.init().await?;

                Ok((event_store, snapshot_store))
            }

            #[cfg(not(feature = "postgres"))]
            {
                Err("PostgreSQL support requires the 'postgres' feature. Rebuild with --features postgres".into())
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
