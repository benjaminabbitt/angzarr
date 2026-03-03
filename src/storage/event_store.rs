//! EventStore trait definition.

use async_trait::async_trait;
use uuid::Uuid;

use super::Result;
use crate::proto::EventPage;

/// Outcome of an `add()` operation.
///
/// Distinguishes between newly added events and duplicates detected via external_id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddOutcome {
    /// Events were persisted. Returns the sequence range of added events.
    Added {
        first_sequence: u32,
        last_sequence: u32,
    },
    /// Duplicate external_id detected. Returns the sequence range of the original events.
    /// No new events were persisted.
    Duplicate {
        first_sequence: u32,
        last_sequence: u32,
    },
}

impl AddOutcome {
    /// Returns true if events were added (not a duplicate).
    pub fn is_added(&self) -> bool {
        matches!(self, AddOutcome::Added { .. })
    }

    /// Returns true if this was a duplicate request.
    pub fn is_duplicate(&self) -> bool {
        matches!(self, AddOutcome::Duplicate { .. })
    }

    /// Returns the first sequence number regardless of outcome.
    pub fn first_sequence(&self) -> u32 {
        match self {
            AddOutcome::Added { first_sequence, .. } => *first_sequence,
            AddOutcome::Duplicate { first_sequence, .. } => *first_sequence,
        }
    }

    /// Returns the last sequence number regardless of outcome.
    pub fn last_sequence(&self) -> u32 {
        match self {
            AddOutcome::Added { last_sequence, .. } => *last_sequence,
            AddOutcome::Duplicate { last_sequence, .. } => *last_sequence,
        }
    }
}

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
/// # Idempotency
///
/// The `external_id` parameter enables exactly-once delivery semantics.
/// When provided (non-empty), the store records a claim for that external_id.
/// Subsequent requests with the same (domain, edition, root, external_id)
/// return `AddOutcome::Duplicate` with the original sequence range instead
/// of persisting duplicate events.
///
/// Pass `None` or empty string for non-idempotent operations.
///
/// Implementations:
/// - `SqliteEventStore`: SQLite storage
/// - `PostgresEventStore`: PostgreSQL storage
/// - `NatsEventStore`: NATS JetStream storage
/// - `MockEventStore`: In-memory mock for testing
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Store events for an aggregate root.
    ///
    /// Events are appended to the existing event stream for this root.
    /// Sequence numbers are validated for consistency.
    /// The correlation_id links related events across aggregates for tracing.
    ///
    /// # Idempotency
    ///
    /// If `external_id` is `Some(id)` where `id` is non-empty:
    /// - First call: persists events and returns `AddOutcome::Added`
    /// - Subsequent calls with same external_id: returns `AddOutcome::Duplicate`
    ///   with the original sequence range (no new events persisted)
    ///
    /// If `external_id` is `None` or `Some("")`: events are always persisted.
    async fn add(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        events: Vec<EventPage>,
        correlation_id: &str,
        external_id: Option<&str>,
    ) -> Result<AddOutcome>;

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

    /// Delete all events for an edition+domain combination.
    ///
    /// Returns the number of events deleted.
    /// Note: This is a destructive operation - events cannot be recovered.
    /// Main timeline ('angzarr' or empty edition) protection must be enforced
    /// by the caller.
    async fn delete_edition_events(&self, domain: &str, edition: &str) -> Result<u32>;
}
