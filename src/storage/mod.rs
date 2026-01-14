//! Storage implementations.

use std::sync::Arc;

use tracing::{error, info};

use crate::config::StorageConfig;
use crate::interfaces::{EventStore, SnapshotStore};

pub mod schema;
pub mod sqlite;

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(feature = "mongodb")]
pub mod mongodb;

pub use sqlite::{SqliteEventStore, SqliteSnapshotStore};

#[cfg(feature = "redis")]
pub use redis::RedisEventStore;

#[cfg(feature = "mongodb")]
pub use mongodb::{MongoEventStore, MongoSnapshotStore};

/// Initialize storage based on configuration.
///
/// Returns tuple of (EventStore, SnapshotStore) implementations based on
/// the configured storage type.
pub async fn init_storage(
    config: &StorageConfig,
) -> Result<(Arc<dyn EventStore>, Arc<dyn SnapshotStore>), Box<dyn std::error::Error>> {
    info!("Storage: {} at {}", config.storage_type, config.path);

    match config.storage_type.as_str() {
        "sqlite" => {
            if let Some(parent) = std::path::Path::new(&config.path).parent() {
                std::fs::create_dir_all(parent)?;
            }

            let pool =
                sqlx::SqlitePool::connect(&format!("sqlite:{}?mode=rwc", config.path)).await?;

            let event_store = Arc::new(SqliteEventStore::new(pool.clone()));
            event_store.init().await?;

            let snapshot_store = Arc::new(SqliteSnapshotStore::new(pool));
            snapshot_store.init().await?;

            Ok((event_store, snapshot_store))
        }
        #[cfg(feature = "mongodb")]
        "mongodb" => {
            let database_name = config.database.as_deref().unwrap_or("angzarr");
            let client = ::mongodb::Client::with_uri_str(&config.path).await?;

            let event_store = Arc::new(MongoEventStore::new(&client, database_name).await?);
            let snapshot_store = Arc::new(MongoSnapshotStore::new(&client, database_name).await?);

            Ok((event_store, snapshot_store))
        }
        #[cfg(not(feature = "mongodb"))]
        "mongodb" => {
            error!("MongoDB storage requested but 'mongodb' feature is not enabled");
            Err("MongoDB feature not enabled".into())
        }
        other => {
            error!("Unknown storage type: {}", other);
            Err(format!("Unknown storage type: {}", other).into())
        }
    }
}
