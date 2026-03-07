// tonic::Status is 176 bytes - acceptable for gRPC error handling
#![allow(clippy::result_large_err)]

//! Unified router for aggregates, sagas, process managers, and projectors.
//!
//! # Overview
//!
//! Two router types based on domain cardinality:
//!
//! - `SingleDomainRouter<S, Mode>`: For aggregates and sagas (one domain, set at construction)
//! - `Router<S, Mode>`: For PMs and projectors (multiple domains via fluent `.domain()`)
//!
//! # Example
//!
//! ```rust,ignore
//! // Aggregate (single domain — domain in constructor)
//! let router = SingleDomainRouter::aggregate("player", "player", PlayerHandler::new());
//!
//! // Saga (single domain — domain in constructor)
//! let router = SingleDomainRouter::saga("saga-order-fulfillment", "order", OrderHandler::new());
//!
//! // Process Manager (multi-domain — fluent .domain())
//! let router = Router::process_manager("pmg-hand-flow", "hand-flow", rebuild_pm_state)
//!     .domain("order", OrderPmHandler::new())
//!     .domain("inventory", InventoryPmHandler::new());
//!
//! // Projector (multi-domain — fluent .domain())
//! let router = Router::projector("prj-output")
//!     .domain("player", PlayerProjectorHandler::new())
//!     .domain("hand", HandProjectorHandler::new());
//! ```

mod cloudevents;
mod dispatch;
mod helpers;
mod saga_context;
mod state;
mod traits;
mod upcaster;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use prost_types::Any;
use tonic::Status;

use crate::proto::{
    business_response, event_page, BusinessResponse, ContextualCommand, Cover, EventBook,
    Notification, ProcessManagerHandleResponse, Projection, RejectionNotification,
    RevocationResponse, SagaResponse,
};

// Re-export public types
pub use helpers::{event_book_from, event_page, new_event_book, new_event_book_multi, pack_event};
pub use saga_context::SagaContext;
pub use state::{EventApplier, StateFactory, StateRouter};
pub use traits::{
    CommandHandlerDomainHandler, CommandRejectedError, CommandResult, ProcessManagerDomainHandler,
    ProcessManagerResponse, ProjectorDomainHandler, RejectionHandlerResponse, SagaDomainHandler,
    SagaHandlerResponse, UnpackAny,
};
pub use upcaster::{BoxedUpcasterHandler, UpcasterHandler, UpcasterMode, UpcasterRouter};

// CloudEvents
pub use cloudevents::{CloudEventsHandler, CloudEventsProjector, CloudEventsRouter};

// Re-export macros (defined in dispatch module via #[macro_export])
pub use crate::dispatch_command;
pub use crate::dispatch_event;

// ============================================================================
// Mode Markers
// ============================================================================

/// Mode marker for command handler routers (commands → events).
pub struct CommandHandlerMode;

/// Mode marker for saga routers (events → commands, stateless).
pub struct SagaMode;

/// Mode marker for process manager routers (events → commands + PM events).
pub struct ProcessManagerMode;

/// Mode marker for projector routers (events → external output).
pub struct ProjectorMode;

// ============================================================================
// SingleDomainRouter — CommandHandler Mode
// ============================================================================

/// Router for command handler components (commands → events, single domain).
///
/// Domain is set at construction time. No `.domain()` method exists,
/// enforcing single-domain constraint at compile time.
pub struct CommandHandlerRouter<S, H>
where
    H: CommandHandlerDomainHandler<State = S>,
{
    name: String,
    domain: String,
    handler: H,
    _state: PhantomData<S>,
}

impl<S: Default + Send + Sync + 'static, H: CommandHandlerDomainHandler<State = S>>
    CommandHandlerRouter<S, H>
{
    /// Create a new command handler router.
    ///
    /// Command handlers accept commands and emit events. Single domain enforced at construction.
    pub fn new(name: impl Into<String>, domain: impl Into<String>, handler: H) -> Self {
        Self {
            name: name.into(),
            domain: domain.into(),
            handler,
            _state: PhantomData,
        }
    }

    /// Get the router name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the domain.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Get command types from the handler.
    pub fn command_types(&self) -> Vec<String> {
        self.handler.command_types()
    }

    /// Get subscriptions for this command handler.
    pub fn subscriptions(&self) -> Vec<(String, Vec<String>)> {
        vec![(self.domain.clone(), self.command_types())]
    }

    /// Rebuild state from events using the handler's state router.
    pub fn rebuild_state(&self, events: &EventBook) -> S {
        self.handler.rebuild(events)
    }

    /// Dispatch a contextual command to the handler.
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
        let state = self.handler.rebuild(event_book);
        let seq = crate::EventBookExt::next_sequence(event_book);

        let type_url = &command_any.type_url;

        // Check for Notification (rejection/compensation)
        if type_url.ends_with("Notification") {
            return dispatch_command_handler_notification(&self.handler, command_any, &state);
        }

        // Execute handler
        let result_book = self
            .handler
            .handle(command_book, command_any, &state, seq)?;

        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(result_book)),
        })
    }
}

/// Dispatch a Notification to the command handler's rejection handler.
fn dispatch_command_handler_notification<S: Default + 'static>(
    handler: &dyn CommandHandlerDomainHandler<State = S>,
    command_any: &Any,
    state: &S,
) -> Result<BusinessResponse, Status> {
    use prost::Message;

    let notification = Notification::decode(command_any.value.as_slice())
        .map_err(|e| Status::invalid_argument(format!("Failed to decode Notification: {}", e)))?;

    let rejection = notification
        .payload
        .as_ref()
        .map(|p| RejectionNotification::decode(p.value.as_slice()))
        .transpose()
        .map_err(|e| {
            Status::invalid_argument(format!("Failed to decode RejectionNotification: {}", e))
        })?
        .unwrap_or_default();

    let (domain, cmd_suffix) = extract_rejection_key(&rejection);

    let response = handler.on_rejected(&notification, state, &domain, &cmd_suffix)?;

    match (response.events, response.notification) {
        (Some(events), _) => Ok(BusinessResponse {
            result: Some(business_response::Result::Events(events)),
        }),
        (None, Some(notif)) => Ok(BusinessResponse {
            result: Some(business_response::Result::Notification(notif)),
        }),
        (None, None) => Ok(BusinessResponse {
            result: Some(business_response::Result::Revocation(RevocationResponse {
                emit_system_revocation: true,
                send_to_dead_letter_queue: false,
                escalate: false,
                abort: false,
                reason: format!(
                    "Handler returned empty response for {}/{}",
                    domain, cmd_suffix
                ),
            })),
        }),
    }
}

// ============================================================================
// SagaRouter — Saga Mode
// ============================================================================

/// Router for saga components (events → commands, single domain, stateless).
///
/// Domain is set at construction time. No `.domain()` method exists,
/// enforcing single-domain constraint at compile time.
pub struct SagaRouter<H>
where
    H: SagaDomainHandler,
{
    name: String,
    domain: String,
    handler: H,
}

impl<H: SagaDomainHandler> SagaRouter<H> {
    /// Create a new saga router.
    ///
    /// Sagas translate events from one domain to commands for another.
    /// Single domain enforced at construction.
    pub fn new(name: impl Into<String>, domain: impl Into<String>, handler: H) -> Self {
        Self {
            name: name.into(),
            domain: domain.into(),
            handler,
        }
    }

    /// Get the router name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the input domain.
    pub fn input_domain(&self) -> &str {
        &self.domain
    }

    /// Get event types from the handler.
    pub fn event_types(&self) -> Vec<String> {
        self.handler.event_types()
    }

    /// Get subscriptions for this saga.
    pub fn subscriptions(&self) -> Vec<(String, Vec<String>)> {
        vec![(self.domain.clone(), self.event_types())]
    }

    /// Dispatch an event to the saga handler.
    ///
    /// Sagas receive only source events — the framework handles sequence
    /// stamping and delivery retries.
    pub fn dispatch(&self, source: &EventBook) -> Result<SagaResponse, Status> {
        let event_page = source
            .pages
            .last()
            .ok_or_else(|| Status::invalid_argument("Source event book has no events"))?;

        let event_any = match &event_page.payload {
            Some(event_page::Payload::Event(e)) => e,
            _ => return Err(Status::invalid_argument("Missing event payload")),
        };

        // Check for Notification (rejection/compensation)
        if event_any.type_url.ends_with("Notification") {
            return dispatch_saga_notification(&self.handler, event_any);
        }

        let response = self.handler.handle(source, event_any)?;

        Ok(SagaResponse {
            commands: response.commands,
            events: response.events,
        })
    }
}

/// Dispatch a Notification to the saga's rejection handler.
fn dispatch_saga_notification<H: SagaDomainHandler>(
    handler: &H,
    event_any: &Any,
) -> Result<SagaResponse, Status> {
    use prost::Message;

    let notification = Notification::decode(event_any.value.as_slice())
        .map_err(|e| Status::invalid_argument(format!("Failed to decode Notification: {}", e)))?;

    let rejection = notification
        .payload
        .as_ref()
        .map(|p| RejectionNotification::decode(p.value.as_slice()))
        .transpose()
        .map_err(|e| {
            Status::invalid_argument(format!("Failed to decode RejectionNotification: {}", e))
        })?
        .unwrap_or_default();

    let (domain, cmd_suffix) = extract_rejection_key(&rejection);

    let response = handler.on_rejected(&notification, &domain, &cmd_suffix)?;

    // Sagas can only return events for compensation (no commands on rejection)
    Ok(SagaResponse {
        commands: vec![],
        events: response.events.into_iter().collect(),
    })
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

// ============================================================================
// ProcessManagerRouter — Process Manager Mode
// ============================================================================

/// Router for process manager components (events → commands + PM events, multi-domain).
///
/// Domains are registered via fluent `.domain()` calls.
pub struct ProcessManagerRouter<S: Default + Send + Sync + 'static> {
    name: String,
    pm_domain: String,
    rebuild: Arc<dyn Fn(&EventBook) -> S + Send + Sync>,
    domains: HashMap<String, Arc<dyn ProcessManagerDomainHandler<S>>>,
}

impl<S: Default + Send + Sync + 'static> ProcessManagerRouter<S> {
    /// Create a new process manager router.
    ///
    /// Process managers correlate events across multiple domains and maintain
    /// their own state. The `pm_domain` is used for storing PM state.
    pub fn new<R>(name: impl Into<String>, pm_domain: impl Into<String>, rebuild: R) -> Self
    where
        R: Fn(&EventBook) -> S + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            pm_domain: pm_domain.into(),
            rebuild: Arc::new(rebuild),
            domains: HashMap::new(),
        }
    }

    /// Register a domain handler.
    ///
    /// Process managers can have multiple input domains.
    pub fn domain<H>(mut self, name: impl Into<String>, handler: H) -> Self
    where
        H: ProcessManagerDomainHandler<S> + 'static,
    {
        self.domains.insert(name.into(), Arc::new(handler));
        self
    }

    /// Get the router name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the PM's own domain (for state storage).
    pub fn pm_domain(&self) -> &str {
        &self.pm_domain
    }

    /// Get subscriptions (domain + event types) for this PM.
    pub fn subscriptions(&self) -> Vec<(String, Vec<String>)> {
        self.domains
            .iter()
            .map(|(domain, handler)| (domain.clone(), handler.event_types()))
            .collect()
    }

    /// Rebuild PM state from events.
    pub fn rebuild_state(&self, events: &EventBook) -> S {
        (self.rebuild)(events)
    }

    /// Get destinations needed for the given trigger and process state.
    pub fn prepare_destinations(
        &self,
        trigger: &Option<EventBook>,
        process_state: &Option<EventBook>,
    ) -> Vec<Cover> {
        let trigger = match trigger {
            Some(t) => t,
            None => return vec![],
        };

        let trigger_domain = trigger
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");

        let event_page = match trigger.pages.last() {
            Some(p) => p,
            None => return vec![],
        };

        let event_any = match &event_page.payload {
            Some(event_page::Payload::Event(e)) => e,
            _ => return vec![],
        };

        let state = match process_state {
            Some(ps) => self.rebuild_state(ps),
            None => S::default(),
        };

        self.domains
            .get(trigger_domain)
            .map(|handler| handler.prepare(trigger, &state, event_any))
            .unwrap_or_default()
    }

    /// Dispatch a trigger event to the appropriate handler.
    pub fn dispatch(
        &self,
        trigger: &EventBook,
        process_state: &EventBook,
        destinations: &[EventBook],
    ) -> Result<ProcessManagerHandleResponse, Status> {
        let trigger_domain = trigger
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");

        let handler = self.domains.get(trigger_domain).ok_or_else(|| {
            Status::unimplemented(format!("No handler for domain: {}", trigger_domain))
        })?;

        let event_page = trigger
            .pages
            .last()
            .ok_or_else(|| Status::invalid_argument("Trigger event book has no events"))?;

        let event_any = match &event_page.payload {
            Some(event_page::Payload::Event(e)) => e,
            _ => return Err(Status::invalid_argument("Missing event payload")),
        };

        let state = self.rebuild_state(process_state);

        // Check for Notification
        if event_any.type_url.ends_with("Notification") {
            return dispatch_pm_notification(handler.as_ref(), event_any, &state);
        }

        let response = handler.handle(trigger, &state, event_any, destinations)?;

        Ok(ProcessManagerHandleResponse {
            commands: response.commands,
            process_events: response.process_events,
            facts: response.facts,
        })
    }
}

/// Dispatch a Notification to the PM's rejection handler.
fn dispatch_pm_notification<S: Default>(
    handler: &dyn ProcessManagerDomainHandler<S>,
    event_any: &Any,
    state: &S,
) -> Result<ProcessManagerHandleResponse, Status> {
    use prost::Message;

    let notification = Notification::decode(event_any.value.as_slice())
        .map_err(|e| Status::invalid_argument(format!("Failed to decode Notification: {}", e)))?;

    let rejection = notification
        .payload
        .as_ref()
        .map(|p| RejectionNotification::decode(p.value.as_slice()))
        .transpose()
        .map_err(|e| {
            Status::invalid_argument(format!("Failed to decode RejectionNotification: {}", e))
        })?
        .unwrap_or_default();

    let (domain, cmd_suffix) = extract_rejection_key(&rejection);

    let response = handler.on_rejected(&notification, state, &domain, &cmd_suffix)?;

    Ok(ProcessManagerHandleResponse {
        commands: vec![],
        process_events: response.events,
        facts: vec![],
    })
}

// ============================================================================
// ProjectorRouter — Projector Mode
// ============================================================================

/// Router for projector components (events → external output, multi-domain).
///
/// Domains are registered via fluent `.domain()` calls.
pub struct ProjectorRouter {
    name: String,
    domains: HashMap<String, Arc<dyn ProjectorDomainHandler>>,
}

impl ProjectorRouter {
    /// Create a new projector router.
    ///
    /// Projectors consume events from multiple domains and produce external output.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            domains: HashMap::new(),
        }
    }

    /// Register a domain handler.
    ///
    /// Projectors can have multiple input domains.
    pub fn domain<H>(mut self, name: impl Into<String>, handler: H) -> Self
    where
        H: ProjectorDomainHandler + 'static,
    {
        self.domains.insert(name.into(), Arc::new(handler));
        self
    }

    /// Get the router name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get subscriptions (domain + event types) for this projector.
    pub fn subscriptions(&self) -> Vec<(String, Vec<String>)> {
        self.domains
            .iter()
            .map(|(domain, handler)| (domain.clone(), handler.event_types()))
            .collect()
    }

    /// Dispatch events to the appropriate handler.
    pub fn dispatch(&self, events: &EventBook) -> Result<Projection, Status> {
        let domain = events
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");

        let handler = self
            .domains
            .get(domain)
            .ok_or_else(|| Status::unimplemented(format!("No handler for domain: {}", domain)))?;

        handler
            .project(events)
            .map_err(|e| Status::internal(e.to_string()))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test mode markers exist
    #[test]
    fn mode_markers_are_zero_sized() {
        assert_eq!(std::mem::size_of::<CommandHandlerMode>(), 0);
        assert_eq!(std::mem::size_of::<SagaMode>(), 0);
        assert_eq!(std::mem::size_of::<ProcessManagerMode>(), 0);
        assert_eq!(std::mem::size_of::<ProjectorMode>(), 0);
    }

    // Test PM router creation
    #[test]
    fn pm_router_creation() {
        let router: ProcessManagerRouter<()> =
            ProcessManagerRouter::new("test-pm", "pm-domain", |_| ());
        assert_eq!(router.name(), "test-pm");
        assert_eq!(router.pm_domain(), "pm-domain");
    }

    // Test projector router creation
    #[test]
    fn projector_router_creation() {
        let router = ProjectorRouter::new("test-prj");
        assert_eq!(router.name(), "test-prj");
    }
}
