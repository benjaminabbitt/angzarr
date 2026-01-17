//! Snapshot storage interface.

use async_trait::async_trait;
use uuid::Uuid;

use super::event_store::Result;
use crate::proto::Snapshot;

/// Interface for snapshot persistence.
///
/// Snapshots are optional optimization to avoid replaying entire event history.
/// When loading an aggregate, if a snapshot exists, events are loaded from
/// the snapshot sequence onwards.
///
/// Implementations:
/// - `MongoSnapshotStore`: MongoDB storage
/// - `PostgresSnapshotStore`: PostgreSQL storage
/// - `EventStoreDbSnapshotStore`: EventStoreDB storage
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
