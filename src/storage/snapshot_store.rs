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
/// Supports multiple snapshots per aggregate for conflict detection with
/// `MergeStrategy::Commutative`. The `put` operation atomically cleans up
/// old transient snapshots (retention = TRANSIENT) when storing a new one.
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
/// - `SqliteSnapshotStore`: SQLite storage
/// - `MockSnapshotStore`: In-memory mock for testing
#[async_trait]
pub trait SnapshotStore: Send + Sync {
    /// Retrieve the latest snapshot for an aggregate.
    ///
    /// Returns `None` if no snapshot exists.
    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Option<Snapshot>>;

    /// Retrieve snapshot at or before a specific sequence.
    ///
    /// Used for conflict detection: loads historical state to compare
    /// field mutations between concurrent commands.
    ///
    /// Returns the snapshot with the highest sequence <= `seq`, or `None`
    /// if no such snapshot exists.
    async fn get_at_seq(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        seq: u32,
    ) -> Result<Option<Snapshot>>;

    /// Store a snapshot for an aggregate.
    ///
    /// Atomically stores the new snapshot and cleans up old transient
    /// snapshots (retention = TRANSIENT) with sequence < this snapshot.
    /// Snapshots with retention = PERSIST are kept indefinitely.
    async fn put(&self, domain: &str, edition: &str, root: Uuid, snapshot: Snapshot) -> Result<()>;

    /// Delete all snapshots for an aggregate.
    async fn delete(&self, domain: &str, edition: &str, root: Uuid) -> Result<()>;
}
