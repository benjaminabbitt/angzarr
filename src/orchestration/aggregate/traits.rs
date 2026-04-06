//! Trait definitions for aggregate command pipeline abstraction.
//!
//! Defines the contracts for aggregate context (storage/persistence) and
//! client logic (business rule invocation).

use std::sync::Arc;

use async_trait::async_trait;
use tonic::Status;
use uuid::Uuid;

use crate::proto::{
    AngzarrDeferredSequence, BusinessResponse, CommandBook, ContextualCommand, EventBook,
    Projection,
};

use super::types::{FactContext, TemporalQuery};

/// Result of persisting events to storage.
#[derive(Debug, Clone)]
pub enum PersistOutcome {
    /// Events were persisted successfully.
    Persisted(EventBook),
    /// No changes to persist (command was a no-op).
    NoOp(EventBook),
    /// Duplicate external_id detected. No new events persisted.
    Duplicate {
        first_sequence: u32,
        last_sequence: u32,
    },
}

/// Context for aggregate command pipeline.
///
/// Implementations provide storage access and post-persist behavior.
/// client logic invocation is always via gRPC and handled by the pipeline.
///
/// All domain-scoped methods take `domain` and `edition` as separate parameters.
/// Domain is the bare aggregate domain (`"order"`, `"cart"`).
/// Edition identifies the timeline (`"angzarr"` for main, named editions for forks).
#[async_trait]
#[allow(clippy::too_many_arguments)]
pub trait AggregateContext: Send + Sync {
    /// Load prior events for the aggregate.
    ///
    /// For edition branches, `explicit_divergence` specifies where the branch
    /// splits from the main timeline. When `None`, uses implicit divergence
    /// (first edition event). Explicit divergence is required for NEW branches
    /// that don't yet have edition events.
    async fn load_prior_events(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        temporal: &TemporalQuery,
    ) -> Result<EventBook, Status> {
        self.load_prior_events_with_divergence(domain, edition, root, temporal, None)
            .await
    }

    /// Load prior events with explicit divergence point for edition branching.
    async fn load_prior_events_with_divergence(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        temporal: &TemporalQuery,
        explicit_divergence: Option<u32>,
    ) -> Result<EventBook, Status>;

    /// Persist new events to storage.
    ///
    /// Compares `prior` (sent to client logic) with `received` (returned by client logic)
    /// to determine what to persist:
    /// - If snapshots differ (by state comparison), persist the new snapshot
    /// - If pages differ (new pages in received), persist the new pages
    /// - If identical, returns `PersistOutcome::NoOp`
    ///
    /// When `external_id` is provided (fact injection), the storage layer
    /// atomically checks for duplicates and returns `PersistOutcome::Duplicate`
    /// if the external_id was already used for this aggregate root.
    async fn persist_events(
        &self,
        prior: &EventBook,
        received: &EventBook,
        domain: &str,
        edition: &str,
        root: Uuid,
        correlation_id: &str,
        external_id: Option<&str>,
    ) -> Result<PersistOutcome, Status>;

    /// Publish to event bus AND call sync projectors via service discovery.
    /// Returns projections from sync projectors.
    async fn post_persist(&self, events: &EventBook) -> Result<Vec<Projection>, Status>;

    /// Optional: pre-validate sequence before loading events (gRPC fast-path).
    /// On mismatch, may return Status with EventBook in details.
    async fn pre_validate_sequence(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _expected: u32,
    ) -> Result<(), Status> {
        Ok(())
    }

    /// Optional: transform events after loading (e.g., upcasting).
    async fn transform_events(
        &self,
        _domain: &str,
        events: EventBook,
    ) -> Result<EventBook, Status> {
        Ok(events)
    }

    /// Optional: send a command to the dead letter queue.
    /// Called when MERGE_MANUAL encounters a sequence mismatch.
    /// Default implementation logs a warning.
    async fn send_to_dlq(
        &self,
        _command: &CommandBook,
        expected_sequence: u32,
        actual_sequence: u32,
        domain: &str,
    ) {
        tracing::warn!(
            domain = %domain,
            expected = expected_sequence,
            actual = actual_sequence,
            "DLQ not configured, dropping command"
        );
    }

    /// Get the cascade ID for 2PC atomic execution, if set.
    ///
    /// When a cascade_id is active, events are persisted with `committed=false`
    /// and the cascade_id stamped on each event. Returns `None` for normal
    /// (non-cascade) command execution.
    fn cascade_id(&self) -> Option<&str> {
        None
    }

    /// Check if a saga-produced command has already been processed.
    ///
    /// For commands with `angzarr_deferred` sequences, checks if events exist
    /// with matching source info (edition, domain, root, sequence).
    /// Returns `Some(events)` if already processed, None if new.
    ///
    /// Default implementation returns None (no idempotency checking).
    async fn check_deferred_idempotency(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _deferred: &AngzarrDeferredSequence,
    ) -> Result<Option<EventBook>, Status> {
        Ok(None)
    }
}

/// Abstraction for aggregate client logic invocation.
///
/// Decouples the command pipeline from the transport used to call client logic.
/// Implementations may use gRPC (over TCP, UDS), in-process trait calls, etc.
#[async_trait]
pub trait ClientLogic: Send + Sync {
    /// Invoke client logic with prior events and a command.
    async fn invoke(&self, cmd: ContextualCommand) -> Result<BusinessResponse, Status>;

    /// Invoke client logic to handle fact events.
    ///
    /// Called when fact events (with ExternalDeferredSequence markers) are injected.
    /// The aggregate updates its state based on the facts and returns
    /// events to persist. The coordinator will assign real sequence numbers.
    ///
    /// Default: Returns the facts unchanged (pass-through).
    async fn invoke_fact(&self, ctx: FactContext) -> Result<EventBook, Status> {
        // Default: pass through facts unchanged
        Ok(ctx.facts)
    }

    /// Replay events to compute state for COMMUTATIVE merge detection.
    ///
    /// Returns the aggregate's state at the given event sequence as a protobuf `Any`.
    /// The state can then be compared field-by-field to detect conflicts.
    ///
    /// Default: Returns Unimplemented, causing COMMUTATIVE to degrade to STRICT behavior.
    async fn replay(&self, events: &EventBook) -> Result<prost_types::Any, Status> {
        let _ = events;
        Err(Status::unimplemented(
            "Replay not implemented. Aggregate must implement replay() for MERGE_COMMUTATIVE field detection.",
        ))
    }
}

/// Factory for creating per-domain aggregate contexts.
///
/// Captures long-lived dependencies (storage, event bus, discovery) and produces
/// a fresh `AggregateContext` for each command execution. Local and gRPC modes
/// provide different implementations.
///
/// One factory per aggregate domain, matching the saga/PM pattern:
/// - `SagaContextFactory` → one per saga
/// - `PMContextFactory` → one per process manager
/// - `AggregateContextFactory` → one per aggregate domain
pub trait AggregateContextFactory: Send + Sync {
    /// Create an aggregate context for command execution.
    fn create(&self) -> Arc<dyn AggregateContext>;

    /// The domain this factory handles.
    fn domain(&self) -> &str;

    /// The client logic for this domain's business rules.
    fn client_logic(&self) -> Arc<dyn ClientLogic>;
}
