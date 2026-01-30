//! EventStore trait definition.

use async_trait::async_trait;
use uuid::Uuid;

use super::Result;
use crate::proto::EventPage;

/// Interface for event persistence.
///
/// All domain-scoped operations take `domain` as their first parameter,
/// followed by `edition`. The edition identifies the timeline: `"angzarr"`
/// for the main timeline, or a named edition (e.g., `"v2"`) for diverged
/// timelines.
///
/// The `(domain, edition, root, sequence)` tuple forms the unique key
/// for stored events.
///
/// Implementations:
/// - `SqliteEventStore`: SQLite storage
/// - `PostgresEventStore`: PostgreSQL storage
/// - `MongoEventStore`: MongoDB storage
/// - `RedisEventStore`: Redis storage
/// - `MockEventStore`: In-memory mock for testing
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Store events for an aggregate root.
    ///
    /// Events are appended to the existing event stream for this root.
    /// Sequence numbers are validated for consistency.
    /// The correlation_id links related events across aggregates for tracing.
    async fn add(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        events: Vec<EventPage>,
        correlation_id: &str,
    ) -> Result<()>;

    /// Retrieve all events for an aggregate.
    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Vec<EventPage>>;

    /// Retrieve events from sequence N onwards.
    async fn get_from(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>>;

    /// Retrieve events in range [from, to).
    async fn get_from_to(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>>;

    /// List all aggregate roots in a domain within an edition.
    async fn list_roots(&self, domain: &str, edition: &str) -> Result<Vec<Uuid>>;

    /// List all domains that have events stored.
    async fn list_domains(&self) -> Result<Vec<String>>;

    /// Get the next sequence number for an aggregate.
    async fn get_next_sequence(&self, domain: &str, edition: &str, root: Uuid) -> Result<u32>;

    /// Retrieve events up to (inclusive) a timestamp.
    ///
    /// Returns events ordered by sequence ASC where created_at <= until.
    /// Used for temporal queries to reconstruct historical state.
    async fn get_until_timestamp(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        until: &str,
    ) -> Result<Vec<EventPage>>;

    /// Retrieve all events with a given correlation ID across all domains.
    ///
    /// Used for tracing related events across aggregates during saga workflows.
    /// Returns EventBooks grouped by domain/root.
    async fn get_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Vec<crate::proto::EventBook>>;
}
