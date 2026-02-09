//! Common utilities for Angzarr example implementations.

use angzarr::proto::{
    event_page::Sequence, CommandBook, CommandPage, Cover, EventBook, EventPage, Snapshot,
    Uuid as ProtoUuid,
};
use prost::Message;
use tonic::Status;

pub mod identity;
pub mod proto;
pub mod proto_name;
pub mod router;
pub mod server;
pub mod state;
pub mod testing;
pub mod validation;

pub use proto_name::{ProtoTypeName, TYPE_URL_PREFIX};
pub use router::{
    Aggregate, CommandHandler, Dispatcher, PmContext, PmEventHandler, PmHandlerResult,
    ProjectionMode, ProjectorEventHandler, Router, SagaEventHandler, AGGREGATE, PROCESS_MANAGER,
    PROJECTOR, SAGA,
};
pub use server::{
    init_tracing, run_aggregate_server, run_process_manager_server, run_projector_server,
    run_saga_server, AggregateLogic, AggregateWrapper, ProcessManagerLogic, ProcessManagerWrapper,
    ProjectorLogic, ProjectorWrapper, SagaLogic, SagaWrapper,
};
pub use state::{rebuild_from_events, StateApplier, StateBuilder};
pub use validation::{
    require_exists, require_non_negative, require_not_empty, require_not_exists, require_positive,
    require_status, require_status_not,
};

// ============================================================================
// Error Types for client logic
// ============================================================================

/// Result type for client logic operations.
pub type Result<T> = std::result::Result<T, BusinessError>;

/// Errors that can occur during client logic operations.
#[derive(Debug, thiserror::Error)]
pub enum BusinessError {
    #[error("client logic rejected command: {0}")]
    Rejected(String),
}

impl From<BusinessError> for Status {
    fn from(err: BusinessError) -> Self {
        match err {
            BusinessError::Rejected(msg) => Status::failed_precondition(msg),
        }
    }
}

/// Get the current timestamp as a protobuf Timestamp.
pub fn now() -> prost_types::Timestamp {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    prost_types::Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}

/// Get the next sequence number for new events based on prior EventBook state.
///
/// Examines the EventBook to find the highest existing sequence:
/// - If pages exist, uses the last page's sequence + 1
/// - If only snapshot exists, uses snapshot.sequence + 1
/// - If empty/None, returns 0
pub fn next_sequence(event_book: Option<&EventBook>) -> u32 {
    let Some(book) = event_book else {
        return 0;
    };

    // Check last event page first (most recent)
    if let Some(last_page) = book.pages.last() {
        if let Some(Sequence::Num(n)) = &last_page.sequence {
            return n + 1;
        }
    }

    // Fall back to snapshot sequence
    // snapshot.sequence is the last event sequence used to create the snapshot
    if let Some(snapshot) = &book.snapshot {
        return snapshot.sequence + 1;
    }

    0
}

// ============================================================================
// Proto Construction Helpers
// ============================================================================

/// Build a single-event EventBook with snapshot state.
///
/// Standard response for command handlers producing one event.
/// The snapshot sequence is set to 0; the framework computes the actual sequence on persist.
pub fn make_event_book(
    cover: Option<Cover>,
    sequence: u32,
    event_type_url: &str,
    event_value: Vec<u8>,
    state_type_url: &str,
    state_value: Vec<u8>,
) -> EventBook {
    EventBook {
        cover,
        snapshot: Some(Snapshot {
            sequence: 0, // Framework computes from pages
            state: Some(prost_types::Any {
                type_url: state_type_url.to_string(),
                value: state_value,
            }),
        }),
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(sequence)),
            event: Some(prost_types::Any {
                type_url: event_type_url.to_string(),
                value: event_value,
            }),
            created_at: Some(now()),
        }],
        ..Default::default()
    }
}

/// Build a single-command CommandBook for saga/process manager output.
pub fn build_command_book(
    domain: &str,
    root: Option<ProtoUuid>,
    correlation_id: &str,
    type_url: &str,
    command: &impl Message,
) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root,
            correlation_id: correlation_id.to_string(),
            edition: None,
        }),
        pages: vec![CommandPage {
            sequence: 0,
            command: Some(prost_types::Any {
                type_url: type_url.to_string(),
                value: command.encode_to_vec(),
            }),
        }],
        ..Default::default()
    }
}

// ============================================================================
// Command/Event Extraction Helpers
// ============================================================================

/// Extract the command payload from a CommandBook.
///
/// Returns the first page's command as a protobuf Any.
pub fn extract_command(command_book: &CommandBook) -> Result<&prost_types::Any> {
    command_book
        .pages
        .first()
        .and_then(|p| p.command.as_ref())
        .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))
}

/// Decode a protobuf command from raw bytes.
///
/// Maps decode errors to BusinessError::Rejected.
pub fn decode_command<T: Message + Default>(data: &[u8]) -> Result<T> {
    T::decode(data).map_err(|e| BusinessError::Rejected(e.to_string()))
}

/// Try to decode an event if its type_url matches the expected suffix.
///
/// Returns None if the type_url doesn't match or decoding fails.
pub fn decode_event<T: Message + Default>(
    event: &prost_types::Any,
    type_suffix: &str,
) -> Option<T> {
    if !event.type_url.ends_with(type_suffix) {
        return None;
    }
    T::decode(event.value.as_slice()).ok()
}

// ============================================================================
// EventBook Metadata
// ============================================================================

/// Metadata extracted from an EventBook's cover.
pub struct EventBookMetadata<'a> {
    pub root: Option<&'a ProtoUuid>,
    pub correlation_id: &'a str,
}

/// Extract root and correlation_id from an EventBook's cover.
pub fn event_book_metadata(book: &EventBook) -> EventBookMetadata<'_> {
    EventBookMetadata {
        root: book.cover.as_ref().and_then(|c| c.root.as_ref()),
        correlation_id: book
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or(""),
    }
}

// ============================================================================
// Saga Helpers
// ============================================================================

/// Convert a root UUID to a string identifier.
///
/// Used by sagas to extract business keys (e.g. order_id) from aggregate root UUIDs.
pub fn root_id_as_string(root: Option<&ProtoUuid>) -> String {
    root.and_then(|r| uuid::Uuid::from_slice(&r.value).ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

// ============================================================================
// Common Error Messages
// ============================================================================

/// Error messages shared across all aggregate services.
pub mod errmsg {
    pub const UNKNOWN_COMMAND: &str = "Unknown command type";
    pub const NO_COMMAND_PAGES: &str = "CommandBook has no pages";
}
