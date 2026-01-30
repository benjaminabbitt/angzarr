//! PositionStore trait definition.

use async_trait::async_trait;

use super::Result;

/// Interface for position tracking.
///
/// Tracks the last-processed event sequence per handler/domain/edition/root.
/// Used by projectors and sagas to resume from their last checkpoint,
/// avoiding reprocessing already-handled events.
///
/// # Key
///
/// Positions are keyed by `(handler, domain, edition, root)`:
/// - `handler`: name of the projector or saga
/// - `domain`: the aggregate domain being tracked
/// - `edition`: the timeline (`"angzarr"` for main, named editions for forks)
/// - `root`: raw bytes identifying the aggregate root
///
/// # Implementations
///
/// - `MongoPositionStore`: MongoDB storage
/// - `PostgresPositionStore`: PostgreSQL storage
/// - `SqlitePositionStore`: SQLite storage
/// - `MockPositionStore`: In-memory mock for testing
#[async_trait]
pub trait PositionStore: Send + Sync {
    /// Get the last-processed sequence for a handler/domain/edition/root.
    ///
    /// Returns `None` if no position has been recorded.
    async fn get(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
    ) -> Result<Option<u32>>;

    /// Store the last-processed sequence for a handler/domain/edition/root.
    ///
    /// Upserts: creates the position if it doesn't exist, updates if it does.
    async fn put(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
        sequence: u32,
    ) -> Result<()>;
}
