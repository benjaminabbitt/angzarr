//! SnapshotStore trait definition.

use async_trait::async_trait;
use uuid::Uuid;

use super::Result;
use crate::proto::Snapshot;

/// Interface for snapshot persistence.
///
/// Snapshots are optional optimization to avoid replaying entire event history.
/// When loading an aggregate, if a snapshot exists, events are loaded from
/// the snapshot sequence onwards.
///
/// All operations take `domain` as their first parameter, followed by
/// `edition` identifying the timeline (`"angzarr"` for main, named editions
/// for diverged timelines).
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
    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Option<Snapshot>>;

    /// Store a snapshot for an aggregate.
    ///
    /// This replaces any existing snapshot for this root.
    async fn put(&self, domain: &str, edition: &str, root: Uuid, snapshot: Snapshot) -> Result<()>;

    /// Delete the snapshot for an aggregate.
    async fn delete(&self, domain: &str, edition: &str, root: Uuid) -> Result<()>;
}
