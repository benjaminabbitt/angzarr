//! Storage implementations.

use std::sync::Arc;

use tracing::info;

use crate::config::{StorageConfig, StorageType};
use crate::interfaces::{EventStore, SnapshotStore};

pub mod mongodb;
pub mod schema;
pub mod sqlite;

pub use mongodb::{MongoEventStore, MongoSnapshotStore};
pub use sqlite::{SqliteEventStore, SqliteSnapshotStore};

/// Initialize storage based on configuration.
///
/// Returns tuple of (EventStore, SnapshotStore) implementations based on
/// the configured storage type.
pub async fn init_storage(
    config: &StorageConfig,
) -> Result<(Arc<dyn EventStore>, Arc<dyn SnapshotStore>), Box<dyn std::error::Error>> {
    match config.storage_type {
        StorageType::Sqlite => {
            info!("Storage: sqlite at {}", config.sqlite.path);

            if let Some(parent) = std::path::Path::new(&config.sqlite.path).parent() {
                std::fs::create_dir_all(parent)?;
            }

            let pool = sqlx::SqlitePool::connect(&format!(
                "sqlite:{}?mode=rwc",
                config.sqlite.path
            ))
            .await?;

            let event_store = Arc::new(SqliteEventStore::new(pool.clone()));
            event_store.init().await?;

            let snapshot_store = Arc::new(SqliteSnapshotStore::new(pool));
            snapshot_store.init().await?;

            Ok((event_store, snapshot_store))
        }
        StorageType::Mongodb => {
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
    }
}
