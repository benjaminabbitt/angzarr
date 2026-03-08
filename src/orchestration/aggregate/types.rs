//! Type definitions for aggregate command pipeline.
//!
//! Contains enums and simple structs used across the aggregate orchestration module.

use crate::proto::{EventBook, Projection};

/// How to load prior events.
#[derive(Debug, Clone)]
pub enum TemporalQuery {
    /// Current state (latest events, snapshot-optimized).
    Current,
    /// Events up to a specific sequence number (inclusive).
    AsOfSequence(u32),
    /// Events up to a specific timestamp.
    AsOfTimestamp(String),
}

/// Pipeline execution mode.
#[derive(Debug, Clone)]
pub enum PipelineMode {
    /// Normal execution: validate → invoke → persist → post-persist.
    Execute,
    /// Speculative: load temporal state → invoke → return (no persist/publish).
    Speculative {
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<String>,
    },
}

/// Context for fact event handling.
///
/// Contains the fact events to record and the aggregate's prior events.
#[derive(Debug, Clone)]
pub struct FactContext {
    /// The fact events to record (with ExternalDeferredSequence markers in PageHeader).
    pub facts: EventBook,
    /// Prior events for this aggregate root (for state reconstruction).
    pub prior_events: Option<EventBook>,
}

/// Response from fact injection.
#[derive(Debug, Clone)]
pub struct FactResponse {
    /// The persisted events (with real sequence numbers).
    pub events: EventBook,
    /// Projections from sync projectors.
    pub projections: Vec<Projection>,
    /// True if this was a duplicate request (external_id already processed).
    pub already_processed: bool,
}
