//! Storage factory with self-registration pattern.
//!
//! Each storage backend registers itself via `inventory::submit!`.
//! The factory iterates registered backends to find one matching the configured type.

use std::sync::Arc;

use futures::future::BoxFuture;

use super::config::StorageConfig;
use super::error::{errmsg, Result, StorageError};
use super::{EventStore, PositionStore, SnapshotStore};

// ============================================================================
// Backend Registration
// ============================================================================

/// Function signature for creating event and snapshot stores.
pub type CreateStoresFn =
    fn(
        &StorageConfig,
    ) -> BoxFuture<'static, Option<Result<(Arc<dyn EventStore>, Arc<dyn SnapshotStore>)>>>;

/// Function signature for creating position stores.
pub type CreatePositionFn =
    fn(&StorageConfig) -> BoxFuture<'static, Option<Result<Arc<dyn PositionStore>>>>;

/// Self-registering storage backend for event and snapshot stores.
///
/// Backends register themselves via `inventory::submit!`:
///
/// ```ignore
/// inventory::submit! {
///     StoresBackend {
///         try_create: |config| {
///             let storage_type = config.storage_type.clone();
///             Box::pin(async move {
///                 if storage_type != "postgres" {
///                     return None;
///                 }
///                 // Create stores...
///                 Some(Ok((event_store, snapshot_store)))
///             })
///         },
///     }
/// }
/// ```
pub struct StoresBackend {
    pub try_create: CreateStoresFn,
}

inventory::collect!(StoresBackend);

/// Self-registering storage backend for position stores.
///
/// Backends register themselves via `inventory::submit!`:
///
/// ```ignore
/// inventory::submit! {
///     PositionBackend {
///         try_create: |config| {
///             let storage_type = config.storage_type.clone();
///             Box::pin(async move {
///                 if storage_type != "postgres" {
///                     return None;
///                 }
///                 // Create position store...
///                 Some(Ok(position_store))
///             })
///         },
///     }
/// }
/// ```
pub struct PositionBackend {
    pub try_create: CreatePositionFn,
}

inventory::collect!(PositionBackend);

// ============================================================================
// Factory Functions
// ============================================================================

/// Initialize storage based on configuration.
///
/// Returns tuple of (EventStore, SnapshotStore) implementations based on
/// the configured storage type.
///
/// Requires the corresponding feature to be enabled:
/// - PostgreSQL: `--features postgres` (included in default)
/// - SQLite: `--features sqlite`
/// - Bigtable: `--features bigtable`
/// - DynamoDB: `--features dynamo`
pub async fn init_storage(
    config: &StorageConfig,
) -> std::result::Result<(Arc<dyn EventStore>, Arc<dyn SnapshotStore>), Box<dyn std::error::Error>>
{
    for backend in inventory::iter::<StoresBackend> {
        if let Some(result) = (backend.try_create)(config).await {
            return result.map_err(|e| e.into());
        }
    }

    Err(Box::new(StorageError::UnknownType(format!(
        "{}{}",
        errmsg::UNKNOWN_TYPE,
        config.storage_type
    ))))
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
/// - Bigtable: `--features bigtable`
/// - DynamoDB: `--features dynamo`
pub async fn init_position_store(
    config: &StorageConfig,
) -> std::result::Result<Arc<dyn PositionStore>, Box<dyn std::error::Error>> {
    for backend in inventory::iter::<PositionBackend> {
        if let Some(result) = (backend.try_create)(config).await {
            return result.map_err(|e| e.into());
        }
    }

    Err(Box::new(StorageError::UnknownType(format!(
        "{}{}",
        errmsg::UNKNOWN_TYPE,
        config.storage_type
    ))))
}
