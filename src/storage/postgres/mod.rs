//! PostgreSQL implementations of storage interfaces.

use std::sync::Arc;

use tracing::info;

use super::factory::{PositionBackend, StoresBackend};
use super::{EventStore, PositionStore, SnapshotStore};

mod event_store;

pub use event_store::PostgresEventStore;

// Position and Snapshot stores use the unified SQL implementation
pub use super::sql::postgres::{PostgresPositionStore, PostgresSnapshotStore};

// ============================================================================
// Self-Registration
// ============================================================================

inventory::submit! {
    StoresBackend {
        try_create: |config| {
            let storage_type = config.storage_type.clone();
            let uri = config.postgres.uri.clone();
            Box::pin(async move {
                if storage_type != "postgres" {
                    return None;
                }

                info!("Storage: postgres at {}", uri);

                let pool = match sqlx::PgPool::connect(&uri).await {
                    Ok(p) => p,
                    Err(e) => return Some(Err(super::error::StorageError::Database(e))),
                };

                if let Err(e) = sqlx::migrate!("migrations/postgres").run(&pool).await {
                    return Some(Err(super::error::StorageError::Database(e.into())));
                }

                let event_store: Arc<dyn EventStore> = Arc::new(PostgresEventStore::new(pool.clone()));
                let snapshot_store: Arc<dyn SnapshotStore> = Arc::new(PostgresSnapshotStore::new(pool));

                Some(Ok((event_store, snapshot_store)))
            })
        },
    }
}

inventory::submit! {
    PositionBackend {
        try_create: |config| {
            let storage_type = config.storage_type.clone();
            let uri = config.postgres.uri.clone();
            Box::pin(async move {
                if storage_type != "postgres" {
                    return None;
                }

                info!("PositionStore: postgres at {}", uri);

                let pool = match sqlx::PgPool::connect(&uri).await {
                    Ok(p) => p,
                    Err(e) => return Some(Err(super::error::StorageError::Database(e))),
                };

                if let Err(e) = sqlx::migrate!("migrations/postgres").run(&pool).await {
                    return Some(Err(super::error::StorageError::Database(e.into())));
                }

                let position_store: Arc<dyn PositionStore> = Arc::new(PostgresPositionStore::new(pool));

                Some(Ok(position_store))
            })
        },
    }
}
