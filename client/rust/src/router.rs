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

/// Function type for rebuilding state from an EventBook.
pub type StateRebuilder<S> = fn(&EventBook) -> S;

/// Command handler function type.
///
/// Takes command book, command Any, current state, and sequence number.
/// Returns EventBook on success or CommandRejectedError on failure.
pub type CommandHandler<S> = fn(&CommandBook, &Any, &S, u32) -> CommandResult<EventBook>;

/// Revocation handler function type.
///
/// Takes notification (containing RejectionNotification payload) and current state.
/// Returns EventBook on success or CommandRejectedError on failure.
///
/// To access rejection details:
/// ```rust,ignore
/// let rejection = notification.payload.as_ref()
///     .map(|p| RejectionNotification::decode(p.value.as_slice()))
///     .transpose()?
///     .unwrap_or_default();
/// ```
pub type RevocationHandler<S> = fn(&Notification, &S) -> CommandResult<EventBook>;

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

impl<S> CommandRouter<S> {
    /// Create a new command router for the given domain.
    pub fn new(domain: impl Into<String>, rebuild: StateRebuilder<S>) -> Self {
        Self {
            domain: domain.into(),
            rebuild,
            handlers: HashMap::new(),
            rejection_handlers: HashMap::new(),
        }
    }

    /// Register a command handler for commands ending with the given suffix.
    pub fn on(mut self, suffix: impl Into<String>, handler: CommandHandler<S>) -> Self {
        self.handlers.insert(suffix.into(), handler);
        self
    }

    /// Register a rejection handler for when a specific command is rejected.
    ///
    /// Called when a saga/PM command targeting the specified domain and command
    /// type is rejected by the target aggregate.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// router.on_rejected("payment", "ProcessPayment", handle_payment_rejected)
    /// ```
    pub fn on_rejected(
        mut self,
        domain: impl Into<String>,
        command: impl Into<String>,
        handler: RevocationHandler<S>,
    ) -> Self {
        let key = format!("{}/{}", domain.into(), command.into());
        self.rejection_handlers.insert(key, handler);
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

        let command_any = command_page
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command"))?;

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
    fn call_rejection_handler(
        &self,
        notification: &Notification,
        state: &S,
        domain: &str,
        cmd_suffix: &str,
    ) -> Result<BusinessResponse, Status> {
        let key = format!("{}/{}", domain, cmd_suffix);

        if let Some(handler) = self.rejection_handlers.get(&key) {
            let result_book = handler(notification, state)?;
            return Ok(BusinessResponse {
                result: Some(business_response::Result::Events(result_book)),
            });
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
            .and_then(|p| p.command.as_ref())
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

/// Event handler function type for sagas.
///
/// Takes source event book, event Any, and destination event books.
/// Returns optional CommandBook (None means no command to emit).
pub type EventHandler = fn(&EventBook, &Any, &[EventBook]) -> CommandResult<Option<CommandBook>>;

/// Multi-command event handler function type for sagas.
///
/// Takes source event book, event Any, and destination event books.
/// Returns a vector of CommandBooks (empty means no commands to emit).
pub type MultiEventHandler = fn(&EventBook, &Any, &[EventBook]) -> CommandResult<Vec<CommandBook>>;

/// Prepare handler function type for sagas.
///
/// Takes source event book and event Any.
/// Returns list of destination covers to fetch.
pub type PrepareHandler = fn(&EventBook, &Any) -> Vec<crate::proto::Cover>;

/// Internal enum to hold either single or multi-command handlers.
enum HandlerType {
    Single(EventHandler),
    Multi(MultiEventHandler),
}

/// Event router for saga handlers.
///
/// Routes events to handlers based on type URL suffix matching.
pub struct EventRouter {
    name: String,
    input_domain: String,
    output_domain: String,
    output_types: Vec<String>,
    handlers: HashMap<String, HandlerType>,
    prepare_handlers: HashMap<String, PrepareHandler>,
}

impl EventRouter {
    /// Create a new event router for a saga.
    ///
    /// - `name`: Saga component name (e.g., "saga-order-fulfillment")
    /// - `input_domain`: Domain this saga subscribes to
    pub fn new(name: impl Into<String>, input_domain: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            input_domain: input_domain.into(),
            output_domain: String::new(),
            output_types: Vec::new(),
            handlers: HashMap::new(),
            prepare_handlers: HashMap::new(),
        }
    }

    /// Register the output domain and command type this saga sends.
    pub fn sends(mut self, domain: impl Into<String>, command_type: impl Into<String>) -> Self {
        self.output_domain = domain.into();
        self.output_types.push(command_type.into());
        self
    }

    /// Register an event handler for events ending with the given suffix.
    pub fn on(mut self, suffix: impl Into<String>, handler: EventHandler) -> Self {
        self.handlers
            .insert(suffix.into(), HandlerType::Single(handler));
        self
    }

    /// Register a multi-command event handler for events ending with the given suffix.
    ///
    /// Use this for sagas that need to emit multiple commands for a single event
    /// (e.g., PotAwarded -> DepositFunds for each winner).
    pub fn on_many(mut self, suffix: impl Into<String>, handler: MultiEventHandler) -> Self {
        self.handlers
            .insert(suffix.into(), HandlerType::Multi(handler));
        self
    }

    /// Register a prepare handler for events ending with the given suffix.
    pub fn prepare(mut self, suffix: impl Into<String>, handler: PrepareHandler) -> Self {
        self.prepare_handlers.insert(suffix.into(), handler);
        self
    }

    /// Get the saga name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the input domain.
    pub fn input_domain(&self) -> &str {
        &self.input_domain
    }

    /// Get the output domain.
    pub fn output_domain(&self) -> &str {
        &self.output_domain
    }

    /// Get the list of registered event type suffixes.
    pub fn event_types(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// Get the list of output command types.
    pub fn output_types(&self) -> &[String] {
        &self.output_types
    }

    /// Get destinations needed for the given source events.
    pub fn prepare_destinations(&self, source: &Option<EventBook>) -> Vec<crate::proto::Cover> {
        let source = match source {
            Some(s) => s,
            None => return vec![],
        };

        let event_page = match source.pages.last() {
            Some(p) => p,
            None => return vec![],
        };

        let event_any = match &event_page.event {
            Some(e) => e,
            None => return vec![],
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

        handler(source, event_any)
    }

    /// Dispatch an event book to the appropriate handler.
    ///
    /// Returns empty vec if no handler matches or handler returns None/empty.
    pub fn dispatch(
        &self,
        event_book: &EventBook,
        destinations: &[EventBook],
    ) -> Result<Vec<CommandBook>, Status> {
        // Get the last event
        let event_page = match event_book.pages.last() {
            Some(p) => p,
            None => return Ok(vec![]),
        };

        let event_any = match &event_page.event {
            Some(e) => e,
            None => return Ok(vec![]),
        };

        // Find handler by suffix
        let type_url = &event_any.type_url;
        let handler = match self
            .handlers
            .iter()
            .find(|(suffix, _)| type_url.ends_with(*suffix))
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
        }
    }
}

/// Helper to create an event page with proper sequence.
pub fn event_page(seq: u32, event: Any) -> EventPage {
    EventPage {
        sequence: Some(event_page::Sequence::Num(seq)),
        event: Some(event),
        created_at: Some(crate::now()),
        external_payload: None,
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
}

/// Process manager handler function type.
///
/// Takes trigger event book, PM's own state, event Any, and destination event books.
/// Returns commands for other aggregates and events for the PM's own domain.
pub type ProcessManagerHandler<S> =
    fn(&EventBook, &S, &Any, &[EventBook]) -> CommandResult<ProcessManagerResponse>;

/// Process manager prepare handler function type.
///
/// Takes trigger event book, PM's own state, and event Any.
/// Returns list of destination covers to fetch.
pub type ProcessManagerPrepareHandler<S> = fn(&EventBook, &S, &Any) -> Vec<crate::proto::Cover>;

/// Process manager state rebuilder function type.
pub type ProcessManagerStateRebuilder<S> = fn(&EventBook) -> S;

/// Process manager router.
///
/// Routes events to handlers based on type URL suffix matching.
/// Unlike sagas, PMs have their own persistent state (rebuilt from events).
pub struct ProcessManagerRouter<S> {
    name: String,
    pm_domain: String,
    input_domains: Vec<String>,
    output_domains: Vec<(String, Vec<String>)>, // domain -> command types
    rebuild: ProcessManagerStateRebuilder<S>,
    handlers: HashMap<String, ProcessManagerHandler<S>>,
    prepare_handlers: HashMap<String, ProcessManagerPrepareHandler<S>>,
}

impl<S> ProcessManagerRouter<S> {
    /// Create a new process manager router.
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
            output_domains: Vec::new(),
            rebuild,
            handlers: HashMap::new(),
            prepare_handlers: HashMap::new(),
        }
    }

    /// Add an input domain this PM subscribes to.
    pub fn subscribes(mut self, domain: impl Into<String>) -> Self {
        self.input_domains.push(domain.into());
        self
    }

    /// Register an output domain and command type this PM sends.
    pub fn sends(mut self, domain: impl Into<String>, command_type: impl Into<String>) -> Self {
        let domain = domain.into();
        let cmd_type = command_type.into();

        if let Some((_, types)) = self.output_domains.iter_mut().find(|(d, _)| d == &domain) {
            types.push(cmd_type);
        } else {
            self.output_domains.push((domain, vec![cmd_type]));
        }
        self
    }

    /// Register an event handler for events ending with the given suffix.
    pub fn on(mut self, suffix: impl Into<String>, handler: ProcessManagerHandler<S>) -> Self {
        self.handlers.insert(suffix.into(), handler);
        self
    }

    /// Register a prepare handler for events ending with the given suffix.
    pub fn prepare(
        mut self,
        suffix: impl Into<String>,
        handler: ProcessManagerPrepareHandler<S>,
    ) -> Self {
        self.prepare_handlers.insert(suffix.into(), handler);
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

        let event_any = match &event_page.event {
            Some(e) => e,
            None => return vec![],
        };

        // Rebuild state from process_state
        let state = match process_state {
            Some(ps) => (self.rebuild)(ps),
            None => (self.rebuild)(&EventBook::default()),
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

        handler(trigger, &state, event_any)
    }

    /// Dispatch a trigger event to the appropriate handler.
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

        let event_any = match &event_page.event {
            Some(e) => e,
            None => return Ok(ProcessManagerResponse::default()),
        };

        // Rebuild state
        let state = (self.rebuild)(process_state);

        // Find handler by suffix
        let type_url = &event_any.type_url;
        let handler = match self
            .handlers
            .iter()
            .find(|(suffix, _)| type_url.ends_with(*suffix))
        {
            Some((_, h)) => h,
            None => return Ok(ProcessManagerResponse::default()),
        };

        // Execute handler
        handler(trigger, &state, event_any, destinations).map_err(Status::from)
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

    /// Register an event applier for events with the given type suffix.
    ///
    /// The handler receives typed events (auto-decoded from protobuf).
    ///
    /// # Type Parameters
    ///
    /// - `E`: The protobuf event type (must implement `prost::Message + Default`)
    ///
    /// # Arguments
    ///
    /// - `suffix`: The type URL suffix to match (e.g., "PlayerRegistered")
    /// - `handler`: Function that takes `(&mut S, E)` and mutates state
    pub fn on<E>(mut self, suffix: impl Into<String>, handler: fn(&mut S, E)) -> Self
    where
        E: prost::Message + Default + 'static,
    {
        let suffix = suffix.into();
        let boxed: EventApplier<S> = Box::new(move |state, bytes| {
            if let Ok(event) = E::decode(bytes) {
                handler(state, event);
            }
        });
        self.handlers.push((suffix, boxed));
        self
    }

    /// Create fresh state and apply all events from pages.
    ///
    /// This is the terminal operation for standalone usage.
    pub fn with_events(&self, pages: &[EventPage]) -> S {
        let mut state = self.create_state();
        for page in pages {
            if let Some(event) = &page.event {
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
    pub fn apply_single(&self, state: &mut S, event_any: &Any) {
        let type_url = &event_any.type_url;
        for (suffix, handler) in &self.handlers {
            if type_url.ends_with(suffix) {
                handler(state, &event_any.value);
                return;
            }
        }
        // Unknown event type - silently ignore (forward compatibility)
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
