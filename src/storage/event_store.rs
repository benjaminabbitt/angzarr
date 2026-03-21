//! EventStore trait definition.

use async_trait::async_trait;
use uuid::Uuid;

use super::Result;
use crate::proto::EventPage;

/// Source tracking info for saga-produced events.
///
/// Used for idempotency: if events exist with matching source info,
/// the saga command was already processed.
#[derive(Debug, Clone, Default)]
pub struct SourceInfo {
    /// Source edition (usually "angzarr")
    pub edition: String,
    /// Source domain (e.g., "order")
    pub domain: String,
    /// Source aggregate root UUID
    pub root: Uuid,
    /// Source event sequence that triggered the saga
    pub seq: u32,
}

impl SourceInfo {
    /// Create new source info from saga origin.
    pub fn new(
        edition: impl Into<String>,
        domain: impl Into<String>,
        root: Uuid,
        seq: u32,
    ) -> Self {
        Self {
            edition: edition.into(),
            domain: domain.into(),
            root,
            seq,
        }
    }

    /// Check if this source info is empty/unset.
    pub fn is_empty(&self) -> bool {
        self.edition.is_empty() && self.domain.is_empty()
    }
}

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
            AddOutcome::Added { first_sequence, .. }
            | AddOutcome::Duplicate { first_sequence, .. } => *first_sequence,
        }
    }

    /// Returns the last sequence number regardless of outcome.
    pub fn last_sequence(&self) -> u32 {
        match self {
            AddOutcome::Added { last_sequence, .. }
            | AddOutcome::Duplicate { last_sequence, .. } => *last_sequence,
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
    ///
    /// # Source Tracking
    ///
    /// If `source_info` is `Some(info)` where info is non-empty:
    /// - Source info is stored with each event for saga provenance tracking
    /// - Enables idempotency checking for saga-produced commands
    #[allow(clippy::too_many_arguments)]
    async fn add(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        events: Vec<EventPage>,
        correlation_id: &str,
        external_id: Option<&str>,
        source_info: Option<&SourceInfo>,
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

    /// Find events by source info (for saga idempotency checking).
    ///
    /// Queries for events within the target aggregate that were produced by
    /// the specified source (saga trigger). Used to detect duplicate saga commands.
    ///
    /// Returns `Some(events)` if matching events exist, `None` if not found.
    async fn find_by_source(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        source_info: &SourceInfo,
    ) -> Result<Option<Vec<EventPage>>>;

    /// Delete all events for an edition+domain combination.
    ///
    /// Returns the number of events deleted.
    /// Note: This is a destructive operation - events cannot be recovered.
    /// Main timeline ('angzarr' or empty edition) protection must be enforced
    /// by the caller.
    async fn delete_edition_events(&self, domain: &str, edition: &str) -> Result<u32>;

    // =========================================================================
    // Cascade (2PC) Query Methods - Phase 5
    // =========================================================================

    /// Query cascade IDs that have uncommitted events older than the threshold.
    ///
    /// Used by the CascadeReaper background job to find stale cascades that need
    /// timeout-based revocation. Returns cascade_ids that:
    /// - Have uncommitted events (committed=false)
    /// - Have a created_at older than the threshold
    /// - Do NOT already have a Confirmation or Revocation event
    ///
    /// # Arguments
    /// * `threshold` - ISO 8601 timestamp string. Events older than this are considered stale.
    async fn query_stale_cascades(&self, threshold: &str) -> Result<Vec<String>>;

    /// Query all participants (aggregates) in a cascade.
    ///
    /// Returns a list of (domain, edition, root, sequences) tuples for all aggregates
    /// that have uncommitted events for the given cascade_id. Used to write
    /// Revocation events for all participants when a cascade times out.
    async fn query_cascade_participants(&self, cascade_id: &str)
        -> Result<Vec<CascadeParticipant>>;
}

/// Information about an aggregate participating in a cascade.
#[derive(Debug, Clone)]
pub struct CascadeParticipant {
    /// Domain name of the aggregate.
    pub domain: String,
    /// Edition (timeline) of the aggregate.
    pub edition: String,
    /// Root UUID of the aggregate.
    pub root: Uuid,
    /// Sequences of uncommitted events for this cascade.
    pub sequences: Vec<u32>,
}
