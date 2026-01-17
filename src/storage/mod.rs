//! Storage implementations.

use std::sync::Arc;

use tracing::info;

use crate::config::{StorageConfig, StorageType};
use crate::interfaces::{EventStore, SnapshotStore};

pub mod eventstoredb;
pub mod mongodb;
pub mod postgres;
pub mod schema;

pub use eventstoredb::{EventStoreDbEventStore, EventStoreDbSnapshotStore};
pub use mongodb::{MongoEventStore, MongoSnapshotStore};
pub use postgres::{PostgresEventStore, PostgresSnapshotStore};

/// Initialize storage based on configuration.
///
/// Returns tuple of (EventStore, SnapshotStore) implementations based on
/// the configured storage type.
pub async fn init_storage(
    config: &StorageConfig,
) -> Result<(Arc<dyn EventStore>, Arc<dyn SnapshotStore>), Box<dyn std::error::Error>> {
    match config.storage_type {
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
        StorageType::Postgres => {
            info!("Storage: postgres at {}", config.postgres.uri);

            let pool = sqlx::PgPool::connect(&config.postgres.uri).await?;

            let event_store = Arc::new(PostgresEventStore::new(pool.clone()));
            event_store.init().await?;

            let snapshot_store = Arc::new(PostgresSnapshotStore::new(pool));
            snapshot_store.init().await?;

            Ok((event_store, snapshot_store))
        }
        StorageType::Eventstoredb => {
            info!(
                "Storage: eventstoredb at {}",
                config.eventstoredb.connection_string
            );

            let event_store = Arc::new(
                EventStoreDbEventStore::new(&config.eventstoredb.connection_string).await?,
            );
            let snapshot_store = Arc::new(
                EventStoreDbSnapshotStore::new(&config.eventstoredb.connection_string).await?,
            );

            Ok((event_store, snapshot_store))
        }
    }
}
