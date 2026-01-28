//! Common utilities for Angzarr example implementations.

use angzarr::proto::{
    business_response, event_page::Sequence, BusinessResponse, CommandBook, CommandPage,
    ContextualCommand, Cover, EventBook, EventPage, Uuid as ProtoUuid,
};
use prost::Message;
use tonic::Status;

pub mod identity;
pub mod proto;
pub mod server;
pub mod state;
pub mod testing;
pub mod validation;

pub use server::{
    init_tracing, run_aggregate_server, run_process_manager_server, run_projector_server,
    run_saga_server, AggregateLogic, AggregateWrapper, ProcessManagerLogic, ProcessManagerWrapper,
    ProjectorLogic, ProjectorWrapper, SagaLogic, SagaWrapper,
};
pub use state::rebuild_from_events;
pub use validation::{
    require_exists, require_non_negative, require_not_empty, require_not_exists, require_positive,
    require_status, require_status_not,
};

// ============================================================================
// Error Types for Business Logic
// ============================================================================

/// Result type for business logic operations.
pub type Result<T> = std::result::Result<T, BusinessError>;

/// Errors that can occur during business logic operations.
#[derive(Debug, thiserror::Error)]
pub enum BusinessError {
    #[error("Business logic rejected command: {0}")]
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
        snapshot: None,
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(sequence)),
            event: Some(prost_types::Any {
                type_url: event_type_url.to_string(),
                value: event_value,
            }),
            created_at: Some(now()),
        }],
        snapshot_state: Some(prost_types::Any {
            type_url: state_type_url.to_string(),
            value: state_value,
        }),
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

/// Process all event pages in an EventBook, dispatching each to a handler function.
///
/// Extracts metadata (root, correlation_id) once and passes it to each handler invocation.
/// The handler returns a `Vec<CommandBook>` for each event (empty vec to skip).
pub fn process_event_pages<F>(book: &EventBook, handler: F) -> Vec<CommandBook>
where
    F: Fn(&prost_types::Any, Option<&ProtoUuid>, &str) -> Vec<CommandBook>,
{
    let meta = event_book_metadata(book);
    book.pages
        .iter()
        .filter_map(|page| page.event.as_ref())
        .flat_map(|event| handler(event, meta.root, meta.correlation_id))
        .collect()
}

// ============================================================================
// Common Error Messages
// ============================================================================

/// Error messages shared across all aggregate services.
pub mod errmsg {
    pub const UNKNOWN_COMMAND: &str = "Unknown command type";
    pub const NO_COMMAND_PAGES: &str = "CommandBook has no pages";
}

/// Create a BusinessError for an unrecognized command type_url.
pub fn unknown_command(type_url: &str) -> BusinessError {
    BusinessError::Rejected(format!("{}: {}", errmsg::UNKNOWN_COMMAND, type_url))
}

// ============================================================================
// Aggregate Dispatch
// ============================================================================

/// Dispatch a command through an aggregate's handler chain.
///
/// Handles common boilerplate: extracting command/events from ContextualCommand,
/// rebuilding state, computing next sequence, and wrapping the EventBook result.
#[allow(clippy::result_large_err)]
pub fn dispatch_aggregate<S>(
    cmd: ContextualCommand,
    rebuild: impl Fn(Option<&EventBook>) -> S,
    dispatch: impl FnOnce(&CommandBook, &prost_types::Any, &S, u32) -> Result<EventBook>,
) -> std::result::Result<BusinessResponse, Status> {
    let command_book = cmd.command.as_ref();
    let prior_events = cmd.events.as_ref();

    let state = rebuild(prior_events);
    let next_seq = next_sequence(prior_events);

    let Some(cb) = command_book else {
        return Err(BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()).into());
    };

    let command_any = extract_command(cb)?;
    let events = dispatch(cb, command_any, &state, next_seq)?;

    Ok(BusinessResponse {
        result: Some(business_response::Result::Events(events)),
    })
}

// ============================================================================
// Macros
// ============================================================================

/// Define an aggregate's struct boilerplate: DOMAIN const, new(), Default.
///
/// Usage: `common::define_aggregate!(CartLogic, "cart");`
#[macro_export]
macro_rules! define_aggregate {
    ($name:ident, $domain:expr) => {
        impl $name {
            pub const DOMAIN: &'static str = $domain;

            pub fn new() -> Self {
                Self
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

/// Define a saga's struct boilerplate: new(), Default, SagaLogic::execute.
///
/// Requires the saga to implement `pub fn handle(&self, book: &EventBook) -> Vec<CommandBook>`.
///
/// Usage: `common::define_saga!(FulfillmentSaga);`
#[macro_export]
macro_rules! define_saga {
    ($name:ident) => {
        impl $name {
            pub fn new() -> Self {
                Self
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $crate::SagaLogic for $name {
            fn execute(
                &self,
                source: &::angzarr::proto::EventBook,
                _destinations: &[::angzarr::proto::EventBook],
            ) -> Vec<::angzarr::proto::CommandBook> {
                self.handle(source)
            }
        }
    };
}

/// Generate public test wrapper methods for aggregate handler functions.
///
/// Two variants:
/// - `fns`: for free-standing handler functions
/// - `methods`: for handler methods on self
///
/// Usage:
/// ```ignore
/// common::expose_handlers!(fns, CartLogic, CartState, rebuild: rebuild_state, [
///     (handle_create_cart_public, handle_create_cart),
/// ]);
///
/// common::expose_handlers!(methods, InventoryLogic, InventoryState, rebuild: rebuild_state, [
///     (handle_initialize_stock_public, handle_initialize_stock),
/// ]);
/// ```
#[macro_export]
macro_rules! expose_handlers {
    (methods, $logic:ident, $state:ty, rebuild: $rebuild:ident,
     [$(($pub_name:ident, $handler:ident)),* $(,)?]) => {
        impl $logic {
            pub fn rebuild_state_public(
                &self,
                event_book: Option<&::angzarr::proto::EventBook>,
            ) -> $state {
                self.$rebuild(event_book)
            }

            $(
                pub fn $pub_name(
                    &self,
                    command_book: &::angzarr::proto::CommandBook,
                    state: &$state,
                    next_seq: u32,
                ) -> $crate::Result<::angzarr::proto::EventBook> {
                    let cmd = $crate::extract_command(command_book)?;
                    self.$handler(command_book, &cmd.value, state, next_seq)
                }
            )*
        }
    };

    (fns, $logic:ident, $state:ty, rebuild: $rebuild:path,
     [$(($pub_name:ident, $handler:path)),* $(,)?]) => {
        impl $logic {
            pub fn rebuild_state_public(
                &self,
                event_book: Option<&::angzarr::proto::EventBook>,
            ) -> $state {
                $rebuild(event_book)
            }

            $(
                pub fn $pub_name(
                    &self,
                    command_book: &::angzarr::proto::CommandBook,
                    state: &$state,
                    next_seq: u32,
                ) -> $crate::Result<::angzarr::proto::EventBook> {
                    let cmd = $crate::extract_command(command_book)?;
                    $handler(command_book, &cmd.value, state, next_seq)
                }
            )*
        }
    };
}
