// Status is the standard gRPC error type from tonic - boxing would change API
#![allow(clippy::result_large_err)]

//! Command and event routing for aggregate and saga handlers.
//!
//! This module provides routing infrastructure for dispatching commands and events
//! to appropriate handlers based on type URL suffix matching.
//!
//! # Example
//!
//! ```rust,ignore
//! use angzarr_client::{CommandRouter, StateRebuilder};
//!
//! struct PlayerState { /* ... */ }
//!
//! fn rebuild_state(event_book: &EventBook) -> PlayerState {
//!     // Rebuild state from events
//! }
//!
//! let router = CommandRouter::new("player", rebuild_state)
//!     .on("RegisterPlayer", handle_register_player)
//!     .on("DepositFunds", handle_deposit_funds);
//! ```

use std::collections::HashMap;

use prost_types::Any;
use tonic::Status;

use crate::proto::{
    business_response, event_page, BusinessResponse, CommandBook, ContextualCommand, EventBook,
    EventPage, Notification, RejectionNotification, RevocationResponse,
};
use crate::{type_url, EventBookExt};

/// Error type for command rejection with a human-readable reason.
#[derive(Debug, Clone)]
pub struct CommandRejectedError {
    pub reason: String,
}

impl CommandRejectedError {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for CommandRejectedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Command rejected: {}", self.reason)
    }
}

impl std::error::Error for CommandRejectedError {}

impl From<CommandRejectedError> for Status {
    fn from(err: CommandRejectedError) -> Self {
        Status::failed_precondition(err.reason)
    }
}

/// Result type for command handlers.
pub type CommandResult<T> = std::result::Result<T, CommandRejectedError>;

/// Response from rejection handlers.
///
/// Handlers may need to:
/// - Emit events to compensate/fix state (EventBook)
/// - Emit notification upstream to propagate rejection
/// - Both
#[derive(Default)]
pub struct RejectionHandlerResponse {
    /// Events to persist to own state (compensation).
    pub events: Option<EventBook>,
    /// Notification to forward upstream.
    pub notification: Option<Notification>,
}

use std::sync::Arc;

/// Function type for rebuilding state from an EventBook.
pub type StateRebuilder<S> = Arc<dyn Fn(&EventBook) -> S + Send + Sync>;

/// Command handler boxed type (supports both closures and function pointers).
pub type CommandHandler<S> =
    Arc<dyn Fn(&CommandBook, &Any, &S, u32) -> CommandResult<EventBook> + Send + Sync>;

/// Revocation handler boxed type.
///
/// Returns `RejectionHandlerResponse` which can contain events (compensation)
/// and/or notification (upstream propagation).
pub type RevocationHandler<S> =
    Arc<dyn Fn(&Notification, &S) -> CommandResult<RejectionHandlerResponse> + Send + Sync>;

/// Command router for aggregate handlers.
///
/// Routes commands to handlers based on type URL suffix matching.
/// Also routes revocation commands to rejection handlers based on domain/command.
pub struct CommandRouter<S> {
    domain: String,
    rebuild: StateRebuilder<S>,
    handlers: HashMap<String, CommandHandler<S>>,
    rejection_handlers: HashMap<String, RevocationHandler<S>>,
}

impl<S: 'static> CommandRouter<S> {
    /// Create a new command router for the given domain.
    pub fn new<R>(domain: impl Into<String>, rebuild: R) -> Self
    where
        R: Fn(&EventBook) -> S + Send + Sync + 'static,
    {
        Self {
            domain: domain.into(),
            rebuild: Arc::new(rebuild),
            handlers: HashMap::new(),
            rejection_handlers: HashMap::new(),
        }
    }

    /// Register a command handler for commands ending with the given suffix.
    ///
    /// Accepts both function pointers and closures:
    /// ```rust,ignore
    /// // Function pointer
    /// .on("CreateTable", handle_create_table)
    /// // Closure
    /// .on("CreateTable", |cb, cmd, state, seq| agg.handle_create(cb, cmd, state, seq))
    /// ```
    pub fn on<H>(mut self, suffix: impl Into<String>, handler: H) -> Self
    where
        H: Fn(&CommandBook, &Any, &S, u32) -> CommandResult<EventBook> + Send + Sync + 'static,
    {
        self.handlers.insert(suffix.into(), Arc::new(handler));
        self
    }

    /// Register a rejection handler for when a specific command is rejected.
    ///
    /// Called when a saga/PM command targeting the specified domain and command
    /// type is rejected by the target aggregate.
    ///
    /// Returns `RejectionHandlerResponse` which can contain:
    /// - Events to compensate local state
    /// - Notification to propagate rejection upstream
    /// - Both
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// router.on_rejected("payment", "ProcessPayment", handle_payment_rejected)
    /// ```
    pub fn on_rejected<H>(
        mut self,
        domain: impl Into<String>,
        command: impl Into<String>,
        handler: H,
    ) -> Self
    where
        H: Fn(&Notification, &S) -> CommandResult<RejectionHandlerResponse> + Send + Sync + 'static,
    {
        let key = format!("{}/{}", domain.into(), command.into());
        self.rejection_handlers.insert(key, Arc::new(handler));
        self
    }

    /// Get the domain this router handles.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Get the list of registered command type suffixes.
    pub fn command_types(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// Rebuild state from an EventBook using the registered state rebuilder.
    ///
    /// This is used by the Replay RPC to compute state from events.
    pub fn rebuild_state(&self, event_book: &EventBook) -> S {
        (self.rebuild)(event_book)
    }

    /// Dispatch a contextual command to the appropriate handler.
    ///
    /// Detects Notification and routes to rejection handlers.
    pub fn dispatch(&self, cmd: &ContextualCommand) -> Result<BusinessResponse, Status> {
        let command_book = cmd
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command book"))?;

        let command_page = command_book
            .pages
            .first()
            .ok_or_else(|| Status::invalid_argument("Missing command page"))?;

        let command_any = match &command_page.payload {
            Some(crate::proto::command_page::Payload::Command(c)) => c,
            _ => return Err(Status::invalid_argument("Missing command")),
        };

        let event_book = cmd
            .events
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing event book"))?;

        // Rebuild state
        let state = (self.rebuild)(event_book);

        let type_url = &command_any.type_url;

        // Check for Notification (rejection/compensation)
        if type_url.ends_with("Notification") {
            return self.dispatch_notification(command_any, &state);
        }

        // Find handler by suffix
        let handler = self
            .handlers
            .iter()
            .find(|(suffix, _)| type_url.ends_with(*suffix))
            .map(|(_, h)| h)
            .ok_or_else(|| Status::unimplemented(format!("No handler for: {}", type_url)))?;

        // Get next sequence
        let seq = event_book.next_sequence();

        // Execute handler
        let result_book = handler(command_book, command_any, &state, seq)?;

        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(result_book)),
        })
    }

    /// Dispatch a Notification to the appropriate rejection handler.
    fn dispatch_notification(
        &self,
        command_any: &Any,
        state: &S,
    ) -> Result<BusinessResponse, Status> {
        use prost::Message;

        // Decode the Notification
        let notification = Notification::decode(command_any.value.as_slice()).map_err(|e| {
            Status::invalid_argument(format!("Failed to decode Notification: {}", e))
        })?;

        // Unpack rejection details from payload
        let rejection = notification
            .payload
            .as_ref()
            .map(|p| RejectionNotification::decode(p.value.as_slice()))
            .transpose()
            .map_err(|e| {
                Status::invalid_argument(format!("Failed to decode RejectionNotification: {}", e))
            })?
            .unwrap_or_default();

        // Extract domain and command type from rejected_command
        let (domain, cmd_suffix) = extract_rejection_key(&rejection);

        // Build dispatch key and call handler
        self.call_rejection_handler(&notification, state, &domain, &cmd_suffix)
    }

    /// Call the appropriate rejection handler.
    ///
    /// Handler returns `RejectionHandlerResponse` with optional events and notification.
    /// - Events are returned in BusinessResponse::Events
    /// - Notification is returned in BusinessResponse::Notification (if events are None)
    /// - If both present, events take precedence (notification is included in response)
    fn call_rejection_handler(
        &self,
        notification: &Notification,
        state: &S,
        domain: &str,
        cmd_suffix: &str,
    ) -> Result<BusinessResponse, Status> {
        let key = format!("{}/{}", domain, cmd_suffix);

        if let Some(handler) = self.rejection_handlers.get(&key) {
            let response = handler(notification, state)?;

            // Return based on what the handler provided
            return match (response.events, response.notification) {
                // Events present - return them (notification handled by framework)
                (Some(events), _) => Ok(BusinessResponse {
                    result: Some(business_response::Result::Events(events)),
                }),
                // Notification only - forward upstream via Revocation response
                (None, Some(notif)) => Ok(BusinessResponse {
                    result: Some(business_response::Result::Notification(notif)),
                }),
                // Neither - delegate to framework
                (None, None) => Ok(BusinessResponse {
                    result: Some(business_response::Result::Revocation(RevocationResponse {
                        emit_system_revocation: true,
                        send_to_dead_letter_queue: false,
                        escalate: false,
                        abort: false,
                        reason: format!("Handler for {} returned empty response", key),
                    })),
                }),
            };
        }

        // Default: delegate to framework
        Ok(BusinessResponse {
            result: Some(business_response::Result::Revocation(RevocationResponse {
                emit_system_revocation: true,
                send_to_dead_letter_queue: false,
                escalate: false,
                abort: false,
                reason: format!(
                    "Aggregate {} has no custom compensation for {}",
                    self.domain, key
                ),
            })),
        })
    }
}

/// Extract domain and command suffix from a RejectionNotification.
fn extract_rejection_key(rejection: &RejectionNotification) -> (String, String) {
    if let Some(rejected) = &rejection.rejected_command {
        let domain = rejected
            .cover
            .as_ref()
            .map(|c| c.domain.clone())
            .unwrap_or_default();

        let cmd_suffix = rejected
            .pages
            .first()
            .and_then(|p| match &p.payload {
                Some(crate::proto::command_page::Payload::Command(c)) => Some(c),
                _ => None,
            })
            .map(|c| {
                c.type_url
                    .rsplit('/')
                    .next()
                    .unwrap_or(&c.type_url)
                    .to_string()
            })
            .unwrap_or_default();

        (domain, cmd_suffix)
    } else {
        (String::new(), String::new())
    }
}

/// Event handler function type for sagas (function pointer).
///
/// Takes source event book, event Any, and destination event books.
/// Returns optional CommandBook (None means no command to emit).
pub type EventHandler = fn(&EventBook, &Any, &[EventBook]) -> CommandResult<Option<CommandBook>>;

/// Multi-command event handler function type for sagas (function pointer).
///
/// Takes source event book, event Any, and destination event books.
/// Returns a vector of CommandBooks (empty means no commands to emit).
pub type MultiEventHandler = fn(&EventBook, &Any, &[EventBook]) -> CommandResult<Vec<CommandBook>>;

/// Prepare handler function type for sagas (function pointer).
///
/// Takes source event book and event Any.
/// Returns list of destination covers to fetch.
pub type PrepareHandler = fn(&EventBook, &Any) -> Vec<crate::proto::Cover>;

/// Event handler closure type for sagas (closure).
pub type EventHandlerFn =
    Arc<dyn Fn(&EventBook, &Any, &[EventBook]) -> CommandResult<Option<CommandBook>> + Send + Sync>;

/// Multi-command event handler closure type for sagas (closure).
pub type MultiEventHandlerFn =
    Arc<dyn Fn(&EventBook, &Any, &[EventBook]) -> CommandResult<Vec<CommandBook>> + Send + Sync>;

/// Prepare handler closure type for sagas (closure).
pub type PrepareHandlerFn = Arc<dyn Fn(&EventBook, &Any) -> Vec<crate::proto::Cover> + Send + Sync>;

/// Internal enum to hold either single or multi-command handlers.
enum HandlerType {
    Single(EventHandler),
    Multi(MultiEventHandler),
    SingleFn(EventHandlerFn),
    MultiFn(MultiEventHandlerFn),
}

/// Prepare handler type - either fn pointer or closure.
enum PrepareType {
    Fn(PrepareHandler),
    Closure(PrepareHandlerFn),
}

/// Unified event router for sagas, process managers, and projectors.
///
/// Uses fluent `.domain().on()` pattern to register handlers with domain context.
/// Subscriptions are auto-derived from registrations.
///
/// # Example (Saga - single domain)
///
/// ```rust,ignore
/// let router = EventRouter::new("saga-table-hand")
///     .domain("table")
///     .on("HandStarted", handle_started);
/// ```
///
/// # Example (Process Manager - multi-domain)
///
/// ```rust,ignore
/// let router = EventRouter::new("pmg-order-flow")
///     .domain("order")
///     .on("OrderCreated", handle_created)
///     .domain("inventory")
///     .on("StockReserved", handle_reserved);
/// ```
///
/// # Example (Projector - multi-domain)
///
/// ```rust,ignore
/// let router = EventRouter::new("prj-output")
///     .domain("player")
///     .on("PlayerRegistered", handle_registered)
///     .domain("hand")
///     .on("CardsDealt", handle_dealt);
/// ```
pub struct EventRouter {
    name: String,
    current_domain: Option<String>,
    /// domain -> handlers
    handlers: HashMap<String, Vec<(String, HandlerType)>>,
    /// domain -> prepare_handlers
    prepare_handlers: HashMap<String, HashMap<String, PrepareType>>,
}

impl EventRouter {
    /// Create a new event router.
    ///
    /// - `name`: Component name (e.g., "saga-order-fulfillment", "pmg-hand-flow")
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            current_domain: None,
            handlers: HashMap::new(),
            prepare_handlers: HashMap::new(),
        }
    }

    /// Create a new event router with a single input domain (backwards compatibility).
    ///
    /// Deprecated: Use `EventRouter::new(name).domain(input_domain)` instead.
    #[deprecated(note = "Use EventRouter::new(name).domain(input_domain) instead")]
    pub fn with_domain(name: impl Into<String>, input_domain: impl Into<String>) -> Self {
        Self::new(name).domain(input_domain)
    }

    /// Set the current domain context for subsequent `.on()` calls.
    pub fn domain(mut self, name: impl Into<String>) -> Self {
        let domain = name.into();
        self.current_domain = Some(domain.clone());
        self.handlers.entry(domain.clone()).or_default();
        self.prepare_handlers.entry(domain).or_default();
        self
    }

    /// Register an event handler for events ending with the given suffix.
    ///
    /// Must be called after `.domain()` to set context.
    pub fn on(mut self, suffix: impl Into<String>, handler: EventHandler) -> Self {
        let domain = self
            .current_domain
            .as_ref()
            .expect("Must call .domain() before .on()");
        self.handlers
            .get_mut(domain)
            .unwrap()
            .push((suffix.into(), HandlerType::Single(handler)));
        self
    }

    /// Register an event handler closure for events ending with the given suffix.
    ///
    /// Must be called after `.domain()` to set context.
    pub fn on_fn<H>(mut self, suffix: impl Into<String>, handler: H) -> Self
    where
        H: Fn(&EventBook, &Any, &[EventBook]) -> CommandResult<Option<CommandBook>>
            + Send
            + Sync
            + 'static,
    {
        let domain = self
            .current_domain
            .as_ref()
            .expect("Must call .domain() before .on_fn()");
        self.handlers
            .get_mut(domain)
            .unwrap()
            .push((suffix.into(), HandlerType::SingleFn(Arc::new(handler))));
        self
    }

    /// Register a multi-command event handler for events ending with the given suffix.
    ///
    /// Use this for sagas that need to emit multiple commands for a single event
    /// (e.g., PotAwarded -> DepositFunds for each winner).
    ///
    /// Must be called after `.domain()` to set context.
    pub fn on_many(mut self, suffix: impl Into<String>, handler: MultiEventHandler) -> Self {
        let domain = self
            .current_domain
            .as_ref()
            .expect("Must call .domain() before .on_many()");
        self.handlers
            .get_mut(domain)
            .unwrap()
            .push((suffix.into(), HandlerType::Multi(handler)));
        self
    }

    /// Register a multi-command event handler closure for events ending with the given suffix.
    ///
    /// Must be called after `.domain()` to set context.
    pub fn on_many_fn<H>(mut self, suffix: impl Into<String>, handler: H) -> Self
    where
        H: Fn(&EventBook, &Any, &[EventBook]) -> CommandResult<Vec<CommandBook>>
            + Send
            + Sync
            + 'static,
    {
        let domain = self
            .current_domain
            .as_ref()
            .expect("Must call .domain() before .on_many_fn()");
        self.handlers
            .get_mut(domain)
            .unwrap()
            .push((suffix.into(), HandlerType::MultiFn(Arc::new(handler))));
        self
    }

    /// Register a prepare handler for events ending with the given suffix.
    ///
    /// Must be called after `.domain()` to set context.
    pub fn prepare(mut self, suffix: impl Into<String>, handler: PrepareHandler) -> Self {
        let domain = self
            .current_domain
            .as_ref()
            .expect("Must call .domain() before .prepare()");
        self.prepare_handlers
            .get_mut(domain)
            .unwrap()
            .insert(suffix.into(), PrepareType::Fn(handler));
        self
    }

    /// Register a prepare handler closure for events ending with the given suffix.
    ///
    /// Must be called after `.domain()` to set context.
    pub fn prepare_fn<H>(mut self, suffix: impl Into<String>, handler: H) -> Self
    where
        H: Fn(&EventBook, &Any) -> Vec<crate::proto::Cover> + Send + Sync + 'static,
    {
        let domain = self
            .current_domain
            .as_ref()
            .expect("Must call .domain() before .prepare_fn()");
        self.prepare_handlers
            .get_mut(domain)
            .unwrap()
            .insert(suffix.into(), PrepareType::Closure(Arc::new(handler)));
        self
    }

    /// Get the component name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Auto-derive subscriptions from registered handlers.
    ///
    /// Returns list of (domain, event_types) tuples.
    pub fn subscriptions(&self) -> Vec<(String, Vec<String>)> {
        self.handlers
            .iter()
            .filter(|(_, handlers)| !handlers.is_empty())
            .map(|(domain, handlers)| {
                let types: Vec<String> =
                    handlers.iter().map(|(suffix, _)| suffix.clone()).collect();
                (domain.clone(), types)
            })
            .collect()
    }

    /// Get the first registered domain (for backwards compatibility).
    #[deprecated(note = "Use subscriptions() instead")]
    pub fn input_domain(&self) -> &str {
        self.handlers
            .keys()
            .next()
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Get all registered event type suffixes across all domains.
    pub fn event_types(&self) -> Vec<String> {
        self.handlers
            .values()
            .flat_map(|handlers| handlers.iter().map(|(suffix, _)| suffix.clone()))
            .collect()
    }

    /// Get destinations needed for the given source events.
    pub fn prepare_destinations(&self, source: &Option<EventBook>) -> Vec<crate::proto::Cover> {
        let source = match source {
            Some(s) => s,
            None => return vec![],
        };

        let source_domain = source
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");

        let event_page = match source.pages.last() {
            Some(p) => p,
            None => return vec![],
        };

        let event_any = match &event_page.payload {
            Some(event_page::Payload::Event(e)) => e,
            _ => return vec![],
        };

        // Find prepare handler by domain and suffix
        let domain_handlers = match self.prepare_handlers.get(source_domain) {
            Some(h) => h,
            None => return vec![],
        };

        let type_url = &event_any.type_url;
        let handler = match domain_handlers
            .iter()
            .find(|(suffix, _)| type_url.ends_with(*suffix))
        {
            Some((_, h)) => h,
            None => return vec![],
        };

        match handler {
            PrepareType::Fn(h) => h(source, event_any),
            PrepareType::Closure(h) => h(source, event_any),
        }
    }

    /// Dispatch an event book to the appropriate handler.
    ///
    /// Routes based on source domain and event type suffix.
    /// Returns empty vec if no handler matches or handler returns None/empty.
    pub fn dispatch(
        &self,
        event_book: &EventBook,
        destinations: &[EventBook],
    ) -> Result<Vec<CommandBook>, Status> {
        let source_domain = event_book
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");

        // Find handlers for this domain
        let domain_handlers = match self.handlers.get(source_domain) {
            Some(h) => h,
            None => return Ok(vec![]),
        };

        // Get the last event
        let event_page = match event_book.pages.last() {
            Some(p) => p,
            None => return Ok(vec![]),
        };

        let event_any = match &event_page.payload {
            Some(event_page::Payload::Event(e)) => e,
            _ => return Ok(vec![]),
        };

        // Find handler by suffix
        let type_url = &event_any.type_url;
        let handler = match domain_handlers
            .iter()
            .find(|(suffix, _)| type_url.ends_with(suffix))
        {
            Some((_, h)) => h,
            None => return Ok(vec![]),
        };

        // Execute handler based on type
        match handler {
            HandlerType::Single(h) => {
                let result = h(event_book, event_any, destinations).map_err(Status::from)?;
                Ok(result.into_iter().collect())
            }
            HandlerType::Multi(h) => h(event_book, event_any, destinations).map_err(Status::from),
            HandlerType::SingleFn(h) => {
                let result = h(event_book, event_any, destinations).map_err(Status::from)?;
                Ok(result.into_iter().collect())
            }
            HandlerType::MultiFn(h) => h(event_book, event_any, destinations).map_err(Status::from),
        }
    }
}

/// Helper to create an event page with proper sequence.
pub fn event_page(seq: u32, event: Any) -> EventPage {
    EventPage {
        sequence: seq,
        created_at: Some(crate::now()),
        payload: Some(event_page::Payload::Event(event)),
    }
}

/// Helper to create an EventBook from command book cover and events.
pub fn event_book_from(command_book: &CommandBook, pages: Vec<EventPage>) -> EventBook {
    EventBook {
        cover: command_book.cover.clone(),
        pages,
        snapshot: None,
        next_sequence: 0,
    }
}

/// Helper to create an EventBook with a single event.
///
/// This is the common pattern for command handlers returning a single event.
pub fn new_event_book(command_book: &CommandBook, seq: u32, event: Any) -> EventBook {
    event_book_from(command_book, vec![event_page(seq, event)])
}

/// Helper to create an EventBook with multiple events.
///
/// Useful for handlers that emit multiple events (e.g., AwardPot + HandComplete).
pub fn new_event_book_multi(
    command_book: &CommandBook,
    start_seq: u32,
    events: Vec<Any>,
) -> EventBook {
    let pages = events
        .into_iter()
        .enumerate()
        .map(|(i, event)| event_page(start_seq + i as u32, event))
        .collect();
    event_book_from(command_book, pages)
}

/// Pack a protobuf message into an Any with the given type URL.
pub fn pack_event<M: prost::Message>(msg: &M, type_name: &str) -> Any {
    Any {
        type_url: type_url(type_name),
        value: msg.encode_to_vec(),
    }
}

/// Helper trait for unpacking Any messages.
pub trait UnpackAny {
    /// Unpack an Any to a specific message type.
    fn unpack<M: prost::Message + Default>(&self) -> Result<M, prost::DecodeError>;
}

impl UnpackAny for Any {
    fn unpack<M: prost::Message + Default>(&self) -> Result<M, prost::DecodeError> {
        M::decode(self.value.as_slice())
    }
}

/// Response from a process manager handler.
#[derive(Default)]
pub struct ProcessManagerResponse {
    /// Commands to send to other aggregates.
    pub commands: Vec<CommandBook>,
    /// Events to persist to the PM's own domain.
    pub process_events: Option<EventBook>,
    /// Notification to forward upstream (for rejection propagation).
    pub notification: Option<Notification>,
}

/// Process manager handler function pointer type.
///
/// Takes trigger event book, PM's own state, event Any, and destination event books.
/// Returns commands for other aggregates and events for the PM's own domain.
pub type ProcessManagerHandler<S> =
    fn(&EventBook, &S, &Any, &[EventBook]) -> CommandResult<ProcessManagerResponse>;

/// Process manager handler closure type (boxed).
pub type ProcessManagerHandlerFn<S> = Arc<
    dyn Fn(&EventBook, &S, &Any, &[EventBook]) -> CommandResult<ProcessManagerResponse>
        + Send
        + Sync,
>;

/// Process manager prepare handler function pointer type.
///
/// Takes trigger event book, PM's own state, and event Any.
/// Returns list of destination covers to fetch.
pub type ProcessManagerPrepareHandler<S> = fn(&EventBook, &S, &Any) -> Vec<crate::proto::Cover>;

/// Process manager prepare handler closure type (boxed).
pub type ProcessManagerPrepareHandlerFn<S> =
    Arc<dyn Fn(&EventBook, &S, &Any) -> Vec<crate::proto::Cover> + Send + Sync>;

/// Process manager state rebuilder function pointer type.
pub type ProcessManagerStateRebuilder<S> = fn(&EventBook) -> S;

/// Process manager state rebuilder closure type (boxed).
pub type ProcessManagerStateRebuilderFn<S> = Arc<dyn Fn(&EventBook) -> S + Send + Sync>;

/// Process manager rejection handler closure type.
///
/// Takes notification and PM's own state.
/// Returns `RejectionHandlerResponse` with events and/or notification.
pub type ProcessManagerRejectionHandler<S> =
    Arc<dyn Fn(&Notification, &S) -> CommandResult<RejectionHandlerResponse> + Send + Sync>;

/// Internal handler type for ProcessManagerRouter.
enum PMHandlerType<S> {
    Fn(ProcessManagerHandler<S>),
    Closure(ProcessManagerHandlerFn<S>),
}

/// Internal prepare handler type for ProcessManagerRouter.
enum PMPrepareType<S> {
    Fn(ProcessManagerPrepareHandler<S>),
    Closure(ProcessManagerPrepareHandlerFn<S>),
}

/// Internal rebuild type for ProcessManagerRouter.
enum PMRebuildType<S> {
    Fn(ProcessManagerStateRebuilder<S>),
    Closure(ProcessManagerStateRebuilderFn<S>),
}

/// Process manager router.
///
/// Routes events to handlers based on type URL suffix matching.
/// Unlike sagas, PMs have their own persistent state (rebuilt from events).
/// Also routes rejection notifications to rejection handlers.
pub struct ProcessManagerRouter<S> {
    name: String,
    pm_domain: String,
    input_domains: Vec<String>,
    rebuild: PMRebuildType<S>,
    handlers: HashMap<String, PMHandlerType<S>>,
    prepare_handlers: HashMap<String, PMPrepareType<S>>,
    rejection_handlers: HashMap<String, ProcessManagerRejectionHandler<S>>,
}

impl<S: 'static> ProcessManagerRouter<S> {
    /// Create a new process manager router with a function pointer rebuilder.
    ///
    /// - `name`: PM component name (e.g., "pm-poker-hand")
    /// - `pm_domain`: The PM's own domain for its state
    pub fn new(
        name: impl Into<String>,
        pm_domain: impl Into<String>,
        rebuild: ProcessManagerStateRebuilder<S>,
    ) -> Self {
        Self {
            name: name.into(),
            pm_domain: pm_domain.into(),
            input_domains: Vec::new(),
            rebuild: PMRebuildType::Fn(rebuild),
            handlers: HashMap::new(),
            prepare_handlers: HashMap::new(),
            rejection_handlers: HashMap::new(),
        }
    }

    /// Create a new process manager router with a closure rebuilder.
    pub fn new_with_rebuild_fn<R>(
        name: impl Into<String>,
        pm_domain: impl Into<String>,
        rebuild: R,
    ) -> Self
    where
        R: Fn(&EventBook) -> S + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            pm_domain: pm_domain.into(),
            input_domains: Vec::new(),
            rebuild: PMRebuildType::Closure(Arc::new(rebuild)),
            handlers: HashMap::new(),
            prepare_handlers: HashMap::new(),
            rejection_handlers: HashMap::new(),
        }
    }

    /// Add an input domain this PM subscribes to.
    pub fn subscribes(mut self, domain: impl Into<String>) -> Self {
        self.input_domains.push(domain.into());
        self
    }

    /// Register an event handler (function pointer) for events ending with the given suffix.
    pub fn on(mut self, suffix: impl Into<String>, handler: ProcessManagerHandler<S>) -> Self {
        self.handlers
            .insert(suffix.into(), PMHandlerType::Fn(handler));
        self
    }

    /// Register an event handler (closure) for events ending with the given suffix.
    pub fn on_fn<H>(mut self, suffix: impl Into<String>, handler: H) -> Self
    where
        H: Fn(&EventBook, &S, &Any, &[EventBook]) -> CommandResult<ProcessManagerResponse>
            + Send
            + Sync
            + 'static,
    {
        self.handlers
            .insert(suffix.into(), PMHandlerType::Closure(Arc::new(handler)));
        self
    }

    /// Register a prepare handler (function pointer) for events ending with the given suffix.
    pub fn prepare(
        mut self,
        suffix: impl Into<String>,
        handler: ProcessManagerPrepareHandler<S>,
    ) -> Self {
        self.prepare_handlers
            .insert(suffix.into(), PMPrepareType::Fn(handler));
        self
    }

    /// Register a prepare handler (closure) for events ending with the given suffix.
    pub fn prepare_fn<H>(mut self, suffix: impl Into<String>, handler: H) -> Self
    where
        H: Fn(&EventBook, &S, &Any) -> Vec<crate::proto::Cover> + Send + Sync + 'static,
    {
        self.prepare_handlers
            .insert(suffix.into(), PMPrepareType::Closure(Arc::new(handler)));
        self
    }

    /// Register a rejection handler for when a specific command is rejected.
    ///
    /// Called when a PM-issued command targeting the specified domain and command
    /// type is rejected by the target aggregate.
    ///
    /// Returns `RejectionHandlerResponse` which can contain:
    /// - Events to persist to PM's own state
    /// - Notification to propagate rejection upstream
    /// - Both
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// router.on_rejected("table", "JoinTable", handle_join_rejected)
    /// ```
    pub fn on_rejected<H>(
        mut self,
        domain: impl Into<String>,
        command: impl Into<String>,
        handler: H,
    ) -> Self
    where
        H: Fn(&Notification, &S) -> CommandResult<RejectionHandlerResponse> + Send + Sync + 'static,
    {
        let key = format!("{}/{}", domain.into(), command.into());
        self.rejection_handlers.insert(key, Arc::new(handler));
        self
    }

    /// Get the PM name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the PM's own domain.
    pub fn pm_domain(&self) -> &str {
        &self.pm_domain
    }

    /// Get the input domains.
    pub fn input_domains(&self) -> &[String] {
        &self.input_domains
    }

    /// Get the list of registered event type suffixes.
    pub fn event_types(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// Get destinations needed for the given trigger and process state.
    pub fn prepare_destinations(
        &self,
        trigger: &Option<EventBook>,
        process_state: &Option<EventBook>,
    ) -> Vec<crate::proto::Cover> {
        let trigger = match trigger {
            Some(t) => t,
            None => return vec![],
        };

        let event_page = match trigger.pages.last() {
            Some(p) => p,
            None => return vec![],
        };

        let event_any = match &event_page.payload {
            Some(event_page::Payload::Event(e)) => e,
            _ => return vec![],
        };

        // Rebuild state from process_state
        let state = match process_state {
            Some(ps) => self.rebuild_state(ps),
            None => self.rebuild_state(&EventBook::default()),
        };

        // Find prepare handler by suffix
        let type_url = &event_any.type_url;
        let handler = match self
            .prepare_handlers
            .iter()
            .find(|(suffix, _)| type_url.ends_with(*suffix))
        {
            Some((_, h)) => h,
            None => return vec![],
        };

        match handler {
            PMPrepareType::Fn(f) => f(trigger, &state, event_any),
            PMPrepareType::Closure(f) => f(trigger, &state, event_any),
        }
    }

    /// Dispatch a trigger event to the appropriate handler.
    ///
    /// Detects Notification (rejection) payloads and routes to rejection handlers.
    pub fn dispatch(
        &self,
        trigger: &EventBook,
        process_state: &EventBook,
        destinations: &[EventBook],
    ) -> Result<ProcessManagerResponse, Status> {
        // Get the last event from trigger
        let event_page = match trigger.pages.last() {
            Some(p) => p,
            None => return Ok(ProcessManagerResponse::default()),
        };

        let event_any = match &event_page.payload {
            Some(event_page::Payload::Event(e)) => e,
            _ => return Ok(ProcessManagerResponse::default()),
        };

        // Rebuild state
        let state = self.rebuild_state(process_state);

        let type_url = &event_any.type_url;

        // Check for Notification (rejection/compensation)
        if type_url.ends_with("Notification") {
            return self.dispatch_notification(event_any, &state);
        }

        // Find handler by suffix
        let handler = match self
            .handlers
            .iter()
            .find(|(suffix, _)| type_url.ends_with(*suffix))
        {
            Some((_, h)) => h,
            None => return Ok(ProcessManagerResponse::default()),
        };

        // Execute handler
        let result = match handler {
            PMHandlerType::Fn(f) => f(trigger, &state, event_any, destinations),
            PMHandlerType::Closure(f) => f(trigger, &state, event_any, destinations),
        };
        result.map_err(Status::from)
    }

    /// Dispatch a Notification to the appropriate rejection handler.
    fn dispatch_notification(
        &self,
        event_any: &Any,
        state: &S,
    ) -> Result<ProcessManagerResponse, Status> {
        use prost::Message;

        // Decode the Notification
        let notification = Notification::decode(event_any.value.as_slice()).map_err(|e| {
            Status::invalid_argument(format!("Failed to decode Notification: {}", e))
        })?;

        // Unpack rejection details from payload
        let rejection = notification
            .payload
            .as_ref()
            .map(|p| RejectionNotification::decode(p.value.as_slice()))
            .transpose()
            .map_err(|e| {
                Status::invalid_argument(format!("Failed to decode RejectionNotification: {}", e))
            })?
            .unwrap_or_default();

        // Extract domain and command type from rejected_command
        let (domain, cmd_suffix) = extract_rejection_key(&rejection);
        let key = format!("{}/{}", domain, cmd_suffix);

        // Call handler if found
        if let Some(handler) = self.rejection_handlers.get(&key) {
            let response = handler(&notification, state)?;
            return Ok(ProcessManagerResponse {
                commands: vec![],
                process_events: response.events,
                notification: response.notification,
            });
        }

        // Default: no handler, return empty response (framework handles)
        Ok(ProcessManagerResponse::default())
    }

    /// Helper to rebuild state using either fn or closure.
    fn rebuild_state(&self, events: &EventBook) -> S {
        match &self.rebuild {
            PMRebuildType::Fn(f) => f(events),
            PMRebuildType::Closure(f) => f(events),
        }
    }
}

// ============================================================================
// StateRouter - fluent state reconstruction
// ============================================================================

/// Event applier function type for StateRouter.
///
/// Takes mutable state reference and event bytes (to be decoded by handler).
pub type EventApplier<S> = Box<dyn Fn(&mut S, &[u8]) + Send + Sync>;

/// Fluent state reconstruction router.
///
/// Provides a builder pattern for registering event appliers with auto-unpacking.
/// Register once at startup, call with_events() per rebuild.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::StateRouter;
/// use prost::Message;
///
/// fn apply_registered(state: &mut PlayerState, event: PlayerRegistered) {
///     state.player_id = format!("player_{}", event.email);
///     state.display_name = event.display_name;
///     state.exists = true;
/// }
///
/// fn apply_deposited(state: &mut PlayerState, event: FundsDeposited) {
///     if let Some(balance) = event.new_balance {
///         state.bankroll = balance.amount;
///     }
/// }
///
/// // Build router once
/// let player_router = StateRouter::<PlayerState>::new()
///     .on::<PlayerRegistered>("PlayerRegistered", apply_registered)
///     .on::<FundsDeposited>("FundsDeposited", apply_deposited);
///
/// // Use per rebuild
/// fn rebuild_state(event_book: &EventBook) -> PlayerState {
///     player_router.with_events(&event_book.pages)
/// }
/// ```
/// Factory function type for creating initial state.
pub type StateFactory<S> = Box<dyn Fn() -> S + Send + Sync>;

pub struct StateRouter<S: Default> {
    handlers: Vec<(String, EventApplier<S>)>,
    factory: Option<StateFactory<S>>,
}

impl<S: Default + 'static> Default for StateRouter<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Default + 'static> StateRouter<S> {
    /// Create a new StateRouter using S::default() for state creation.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            factory: None,
        }
    }

    /// Create a StateRouter with a custom state factory.
    ///
    /// Use this when your state needs non-default initialization.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn new_hand_state() -> HandState {
    ///     HandState {
    ///         pots: vec![PotState { pot_type: "main".to_string(), ..Default::default() }],
    ///         ..Default::default()
    ///     }
    /// }
    ///
    /// let router = StateRouter::with_factory(new_hand_state)
    ///     .on::<CardsDealt>("CardsDealt", apply_cards_dealt);
    /// ```
    pub fn with_factory(factory: fn() -> S) -> Self {
        Self {
            handlers: Vec::new(),
            factory: Some(Box::new(factory)),
        }
    }

    /// Create a new state instance using factory or Default.
    fn create_state(&self) -> S {
        match &self.factory {
            Some(factory) => factory(),
            None => S::default(),
        }
    }

    /// Register an event applier for the given protobuf event type.
    ///
    /// The handler receives typed events (auto-decoded from protobuf).
    /// Type name is extracted via reflection using `prost::Name::full_name()`.
    ///
    /// # Type Parameters
    ///
    /// - `E`: The protobuf event type (must implement `prost::Message + Default + prost::Name`)
    ///
    /// # Arguments
    ///
    /// - `handler`: Function that takes `(&mut S, E)` and mutates state
    pub fn on<E>(mut self, handler: fn(&mut S, E)) -> Self
    where
        E: prost::Message + Default + prost::Name + 'static,
    {
        let type_name = E::full_name();
        let boxed: EventApplier<S> = Box::new(move |state, bytes| {
            if let Ok(event) = E::decode(bytes) {
                handler(state, event);
            }
        });
        self.handlers.push((type_name, boxed));
        self
    }

    /// Create fresh state and apply all events from pages.
    ///
    /// This is the terminal operation for standalone usage.
    pub fn with_events(&self, pages: &[EventPage]) -> S {
        let mut state = self.create_state();
        for page in pages {
            if let Some(event_page::Payload::Event(event)) = &page.payload {
                self.apply_single(&mut state, event);
            }
        }
        state
    }

    /// Create fresh state and apply all events from an EventBook.
    pub fn with_event_book(&self, event_book: &EventBook) -> S {
        self.with_events(&event_book.pages)
    }

    /// Apply a single event to existing state.
    ///
    /// The suffix must match exactly at a word boundary (after '/' or '.').
    /// For example, suffix "CardsDealt" will NOT match "CommunityCardsDealt".
    pub fn apply_single(&self, state: &mut S, event_any: &Any) {
        let type_url = &event_any.type_url;
        for (suffix, handler) in &self.handlers {
            if Self::type_matches(type_url, suffix) {
                handler(state, &event_any.value);
                return;
            }
        }
        // Unknown event type - silently ignore (forward compatibility)
    }

    /// Check if type_url exactly matches the given fully qualified type name.
    ///
    /// type_name should be fully qualified (e.g., "examples.CardsDealt").
    /// Compares type_url == "type.googleapis.com/" + type_name.
    fn type_matches(type_url: &str, type_name: &str) -> bool {
        type_url == format!("type.googleapis.com/{}", type_name)
    }

    /// Convert to a StateRebuilder function for use with CommandRouter.
    ///
    /// Returns a function pointer that can be passed to CommandRouter::new().
    ///
    /// Note: This requires the StateRouter to be stored in a static variable
    /// since CommandRouter expects a function pointer.
    pub fn into_rebuilder(self) -> impl Fn(&EventBook) -> S + Send + Sync {
        move |event_book| self.with_event_book(event_book)
    }
}

// ============================================================================
// UpcasterRouter — event version transformation
// ============================================================================

/// Handler type for upcasting events from old versions to new versions.
///
/// Takes an old event (Any) and returns the new event (Any).
pub type UpcasterHandler = Arc<dyn Fn(&Any) -> Any + Send + Sync>;

/// Event version transformer.
///
/// Matches old event type_url suffixes and transforms to new versions.
/// Events without registered transformations pass through unchanged.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::UpcasterRouter;
///
/// fn upcast_created_v1(old: &Any) -> Any {
///     let v1: OrderCreatedV1 = old.unpack().unwrap();
///     let v2 = OrderCreated {
///         order_id: v1.order_id,
///         total: 0, // default for new field
///     };
///     pack_any(&v2)
/// }
///
/// let router = UpcasterRouter::new("order")
///     .on("OrderCreatedV1", upcast_created_v1);
///
/// let new_events = router.upcast(&old_events);
/// ```
pub struct UpcasterRouter {
    domain: String,
    handlers: Vec<(String, UpcasterHandler)>,
}

impl UpcasterRouter {
    /// Create a new upcaster router for a domain.
    pub fn new(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            handlers: Vec::new(),
        }
    }

    /// Register a handler for an old event type_url suffix.
    ///
    /// # Arguments
    /// - `suffix`: The type_url suffix to match (e.g., "OrderCreatedV1")
    /// - `handler`: Function that transforms old event to new event
    pub fn on<F>(mut self, suffix: impl Into<String>, handler: F) -> Self
    where
        F: Fn(&Any) -> Any + Send + Sync + 'static,
    {
        self.handlers.push((suffix.into(), Arc::new(handler)));
        self
    }

    /// Transform a list of events to current versions.
    ///
    /// Events matching registered handlers are transformed.
    /// Events without matching handlers pass through unchanged.
    pub fn upcast(&self, events: &[EventPage]) -> Vec<EventPage> {
        events
            .iter()
            .map(|page| {
                let Some(event_page::Payload::Event(event)) = &page.payload else {
                    return page.clone();
                };

                for (suffix, handler) in &self.handlers {
                    if event.type_url.ends_with(suffix) {
                        let new_event = handler(event);
                        let mut new_page = page.clone();
                        new_page.payload = Some(event_page::Payload::Event(new_event));
                        return new_page;
                    }
                }

                page.clone()
            })
            .collect()
    }

    /// Get the domain this upcaster handles.
    pub fn domain(&self) -> &str {
        &self.domain
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    // =========================================================================
    // RejectionHandlerResponse Tests
    // =========================================================================

    #[test]
    fn empty_response_has_no_events_or_notification() {
        let response = RejectionHandlerResponse::default();

        assert!(response.events.is_none());
        assert!(response.notification.is_none());
    }

    #[test]
    fn response_with_events_only() {
        let event_book = make_event_book();

        let response = RejectionHandlerResponse {
            events: Some(event_book),
            notification: None,
        };

        assert!(response.events.is_some());
        assert_eq!(response.events.as_ref().unwrap().pages.len(), 1);
        assert!(response.notification.is_none());
    }

    #[test]
    fn response_with_notification_only() {
        let notification = make_notification("inventory", "ReserveStock", "out of stock");

        let response = RejectionHandlerResponse {
            events: None,
            notification: Some(notification),
        };

        assert!(response.events.is_none());
        assert!(response.notification.is_some());
    }

    #[test]
    fn response_with_both_events_and_notification() {
        let event_book = make_event_book();
        let notification = make_notification("payment", "ProcessPayment", "declined");

        let response = RejectionHandlerResponse {
            events: Some(event_book),
            notification: Some(notification),
        };

        assert!(response.events.is_some());
        assert!(response.notification.is_some());
    }

    #[test]
    fn response_events_are_accessible() {
        let mut event_book = EventBook::default();
        event_book.pages.push(EventPage {
            payload: Some(event_page::Payload::Event(Any {
                type_url: "type.googleapis.com/test.Event1".to_string(),
                value: vec![],
            })),
            ..Default::default()
        });
        event_book.pages.push(EventPage {
            payload: Some(event_page::Payload::Event(Any {
                type_url: "type.googleapis.com/test.Event2".to_string(),
                value: vec![],
            })),
            ..Default::default()
        });

        let response = RejectionHandlerResponse {
            events: Some(event_book),
            notification: None,
        };

        assert_eq!(response.events.as_ref().unwrap().pages.len(), 2);
    }

    // =========================================================================
    // Helper Functions
    // =========================================================================

    fn make_event_book() -> EventBook {
        let mut book = EventBook::default();
        book.pages.push(EventPage {
            payload: Some(event_page::Payload::Event(Any {
                type_url: "type.googleapis.com/test.TestEvent".to_string(),
                value: vec![],
            })),
            ..Default::default()
        });
        book
    }

    fn make_notification(domain: &str, command_type: &str, reason: &str) -> Notification {
        use crate::proto::{command_page, CommandBook, CommandPage, Cover};

        let mut rejected_command = CommandBook::default();
        rejected_command.cover = Some(Cover {
            domain: domain.to_string(),
            ..Default::default()
        });
        rejected_command.pages.push(CommandPage {
            payload: Some(command_page::Payload::Command(Any {
                type_url: format!("type.googleapis.com/test.{}", command_type),
                value: vec![],
            })),
            ..Default::default()
        });

        let rejection = RejectionNotification {
            issuer_name: "test-saga".to_string(),
            issuer_type: "saga".to_string(),
            rejection_reason: reason.to_string(),
            rejected_command: Some(rejected_command),
            ..Default::default()
        };

        Notification {
            payload: Some(Any {
                type_url: "type.googleapis.com/angzarr.RejectionNotification".to_string(),
                value: rejection.encode_to_vec(),
            }),
            ..Default::default()
        }
    }

    // =========================================================================
    // StateRouter Tests
    // =========================================================================

    #[test]
    fn type_matches_requires_fully_qualified_name() {
        // Fully qualified name matches
        assert!(StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.CardsDealt",
            "examples.CardsDealt"
        ));
        // Unqualified name does NOT match (must be fully qualified)
        assert!(!StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.CardsDealt",
            "CardsDealt"
        ));
    }

    #[test]
    fn type_matches_rejects_partial_names() {
        // "CardsDealt" should NOT match "CommunityCardsDealt"
        assert!(!StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.CommunityCardsDealt",
            "examples.CardsDealt"
        ));
        // Only exact match works
        assert!(StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.CommunityCardsDealt",
            "examples.CommunityCardsDealt"
        ));
    }

    #[test]
    fn type_matches_rejects_wrong_package() {
        // Wrong package does not match
        assert!(!StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.CardsDealt",
            "other.CardsDealt"
        ));
    }

    #[test]
    fn type_matches_handles_edge_cases() {
        // Empty type name does not match
        assert!(!StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.Test",
            ""
        ));
        // Completely different type does not match
        assert!(!StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.Other",
            "examples.CardsDealt"
        ));
    }

    // =========================================================================
    // UpcasterRouter Tests
    // =========================================================================

    #[test]
    fn upcaster_router_new_creates_router() {
        let router = UpcasterRouter::new("order");
        assert_eq!(router.domain(), "order");
    }

    #[test]
    fn upcaster_router_on_chains_fluently() {
        // Just verify chaining works - no types() assertion
        let _router = UpcasterRouter::new("order")
            .on("OrderCreatedV1", |_| Any::default())
            .on("OrderShippedV1", |_| Any::default());
    }

    #[test]
    fn upcaster_router_upcast_transforms_matching_events() {
        let router = UpcasterRouter::new("order").on("TestEventV1", |_old| Any {
            type_url: "type.googleapis.com/test.TestEventV2".to_string(),
            value: vec![1, 2, 3],
        });

        let old_events = vec![EventPage {
            payload: Some(event_page::Payload::Event(Any {
                type_url: "type.googleapis.com/test.TestEventV1".to_string(),
                value: vec![],
            })),
            ..Default::default()
        }];

        let new_events = router.upcast(&old_events);

        assert_eq!(new_events.len(), 1);
        if let Some(event_page::Payload::Event(event)) = &new_events[0].payload {
            assert!(event.type_url.ends_with("TestEventV2"));
            assert_eq!(event.value, vec![1, 2, 3]);
        } else {
            panic!("Expected event payload");
        }
    }

    #[test]
    fn upcaster_router_upcast_passes_through_unmatched_events() {
        let router = UpcasterRouter::new("order").on("OrderCreatedV1", |_| Any::default());

        let events = vec![EventPage {
            payload: Some(event_page::Payload::Event(Any {
                type_url: "type.googleapis.com/test.OtherEvent".to_string(),
                value: vec![42],
            })),
            ..Default::default()
        }];

        let result = router.upcast(&events);

        assert_eq!(result.len(), 1);
        if let Some(event_page::Payload::Event(event)) = &result[0].payload {
            assert!(event.type_url.ends_with("OtherEvent"));
            assert_eq!(event.value, vec![42]);
        } else {
            panic!("Expected event payload");
        }
    }

    #[test]
    fn upcaster_router_upcast_handles_empty_input() {
        let router = UpcasterRouter::new("order");
        let result = router.upcast(&[]);
        assert!(result.is_empty());
    }
}
