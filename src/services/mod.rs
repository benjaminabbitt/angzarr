//! gRPC service implementations.

pub mod aggregate;
pub mod event_query;
pub mod gap_fill;
pub mod projector_coord;
pub mod snapshot_handler;
pub mod upcaster;

/// Error message constants for gRPC services.
pub mod errmsg {
    /// CommandRequest missing command.
    pub const COMMAND_REQUEST_MISSING_COMMAND: &str = "CommandRequest must have a command";
    /// SpeculateAggregateRequest missing command.
    pub const SPECULATE_AGG_MISSING_COMMAND: &str = "SpeculateAggregateRequest must have a command";
    /// EventRequest missing events.
    pub const EVENT_REQUEST_MISSING_EVENTS: &str = "EventRequest must have events";
    /// SpeculateProjectorRequest missing events.
    pub const SPECULATE_PROJ_MISSING_EVENTS: &str = "SpeculateProjectorRequest requires events";
    /// Query missing cover with domain/root or correlation.
    pub const QUERY_MISSING_COVER_OR_CORRELATION: &str =
        "Query must have a cover with domain/root or correlation_id";
    /// Query missing root UUID or correlation.
    pub const QUERY_MISSING_ROOT_OR_CORRELATION: &str =
        "Query must have a root UUID or correlation_id";
    /// Query missing cover.
    pub const QUERY_MISSING_COVER: &str = "Query must have a cover";
    /// Query missing root UUID.
    pub const QUERY_MISSING_ROOT: &str = "Query must have a root UUID";
    /// Invalid UUID prefix.
    pub const INVALID_UUID: &str = "Invalid UUID: ";
    /// Temporal query missing point in time.
    pub const TEMPORAL_QUERY_MISSING_POINT: &str =
        "TemporalQuery must specify as_of_time or as_of_sequence";
    /// EventBook gap-fill failure prefix.
    pub const REPAIR_EVENTBOOK_FAILED: &str = "Failed to fill EventBook gaps: ";
    /// Projector failure suffix (appears after projector name).
    pub const PROJECTOR_FAILED: &str = " failed: ";
}

pub use crate::utils::saga_compensation::{
    build_compensation_failed_event, build_compensation_failed_event_book,
    build_notification_command_book, build_rejection_notification, handle_business_response,
    CompensationContext, CompensationError, CompensationOutcome, DefaultEscalationHandler,
    EscalationHandler, NoopEscalationHandler,
};
pub use aggregate::AggregateService;
pub use event_query::EventQueryService;
pub use gap_fill::{
    GapFiller, LocalEventSource, NoOpPositionStore, PositionStoreAdapter, RemoteEventSource,
};
pub use projector_coord::ProjectorCoord;
pub use upcaster::{Upcaster, UpcasterConfig};
