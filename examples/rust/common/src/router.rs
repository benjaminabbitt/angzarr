//! DRY dispatch via router types.
//!
//! - `Dispatcher<H>`: Single-domain handler dispatch (base building block)
//! - `Router<H>`: Wraps one or more dispatchers (component-level)
//! - `Aggregate<S>`: Aggregate handler (Router + state rebuild)
//!
//! All auto-derive `ComponentDescriptor` from their registrations.

// ============================================================================
// Component type constants
// ============================================================================

pub const AGGREGATE: &str = "aggregate";
pub const SAGA: &str = "saga";
pub const PROJECTOR: &str = "projector";
pub const PROCESS_MANAGER: &str = "process_manager";

use angzarr::proto::{
    business_response, BusinessResponse, CommandBook, ComponentDescriptor, ContextualCommand,
    EventBook, Target, Uuid as ProtoUuid,
};
use tonic::Status;

use crate::{errmsg, event_book_metadata, extract_command, next_sequence, BusinessError, Result};

// ============================================================================
// Dispatcher<H> — single-domain handler dispatch
// ============================================================================

/// Single-domain dispatcher. Matches type_url suffixes to handlers.
///
/// Building block for routers. Each dispatcher handles one domain.
pub struct Dispatcher<H> {
    domain: &'static str,
    handlers: Vec<(&'static str, H)>,
}

impl<H> Dispatcher<H> {
    /// Create a dispatcher for a domain.
    pub fn new(domain: &'static str) -> Self {
        Self {
            domain,
            handlers: Vec::new(),
        }
    }

    /// Register a handler for a type_url suffix.
    pub fn on(mut self, type_suffix: &'static str, handler: H) -> Self {
        self.handlers.push((type_suffix, handler));
        self
    }

    /// Get the domain this dispatcher handles.
    pub fn domain(&self) -> &'static str {
        self.domain
    }

    /// Get registered event/command type suffixes.
    pub fn types(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.handlers.iter().map(|(s, _)| *s)
    }

    /// Find handler for a type_url (first suffix match).
    pub fn find_handler(&self, type_url: &str) -> Option<&H> {
        self.handlers
            .iter()
            .find(|(suffix, _)| type_url.ends_with(suffix))
            .map(|(_, h)| h)
    }
}

// ============================================================================
// Router<H> — multi-dispatcher component wrapper
// ============================================================================

/// Component router wrapping one or more domain dispatchers.
///
/// Provides component metadata (name, type) and combines dispatchers
/// for descriptor generation and handler lookup.
pub struct Router<H> {
    name: &'static str,
    component_type: &'static str,
    state_domain: Option<&'static str>,
    dispatchers: Vec<Dispatcher<H>>,
}

impl<H> Router<H> {
    /// Create a router with component metadata.
    pub fn new(name: &'static str, component_type: &'static str) -> Self {
        Self {
            name,
            component_type,
            state_domain: None,
            dispatchers: Vec::new(),
        }
    }

    /// Set the state domain (for stateful components like PMs).
    pub fn state_domain(mut self, domain: &'static str) -> Self {
        self.state_domain = Some(domain);
        self
    }

    /// Add a dispatcher for a domain.
    pub fn with(mut self, dispatcher: Dispatcher<H>) -> Self {
        self.dispatchers.push(dispatcher);
        self
    }

    /// Build ComponentDescriptor from registered dispatchers.
    pub fn descriptor(&self) -> ComponentDescriptor {
        ComponentDescriptor {
            name: self.name.to_string(),
            component_type: self.component_type.to_string(),
            inputs: self
                .dispatchers
                .iter()
                .map(|d| Target {
                    domain: d.domain().to_string(),
                    types: d.types().map(|s| s.to_string()).collect(),
                })
                .collect(),
        }
    }

    /// Get the component name.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Get the first input domain (for single-domain components).
    pub fn input_domain(&self) -> &'static str {
        self.dispatchers
            .first()
            .map(|d| d.domain())
            .unwrap_or("")
    }

    /// Get the state domain (for stateful components).
    pub fn get_state_domain(&self) -> Option<&'static str> {
        self.state_domain
    }

    /// Find handler across all dispatchers (first match).
    fn find_handler(&self, type_url: &str) -> Option<&H> {
        self.dispatchers
            .iter()
            .find_map(|d| d.find_handler(type_url))
    }
}

// ============================================================================
// Router<SagaEventHandler> — saga dispatch
// ============================================================================

/// Handler for saga events. Returns commands to execute.
pub type SagaEventHandler = fn(&prost_types::Any, Option<&ProtoUuid>, &str) -> Vec<CommandBook>;

impl Router<SagaEventHandler> {
    /// Dispatch events to handlers, collect commands.
    pub fn dispatch(&self, book: &EventBook) -> Vec<CommandBook> {
        let meta = event_book_metadata(book);
        book.pages
            .iter()
            .filter_map(|page| page.event.as_ref())
            .flat_map(|event| {
                if let Some(handler) = self.find_handler(&event.type_url) {
                    handler(event, meta.root, meta.correlation_id)
                } else {
                    vec![]
                }
            })
            .collect()
    }
}

// ============================================================================
// Router<ProjectorEventHandler> — projector dispatch
// ============================================================================

/// Projection execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionMode {
    /// Normal execution: compute and persist.
    Execute,
    /// Speculative: compute only, skip persistence.
    Speculate,
}

/// Handler for projector events. Returns optional projection data.
pub type ProjectorEventHandler =
    fn(&prost_types::Any, Option<&ProtoUuid>, &str, ProjectionMode) -> Result<Option<prost_types::Any>>;

impl Router<ProjectorEventHandler> {
    /// Dispatch events to handlers, return projection.
    pub fn dispatch(
        &self,
        book: &EventBook,
        mode: ProjectionMode,
    ) -> Result<angzarr::proto::Projection> {
        let meta = event_book_metadata(book);
        let mut projection_data: Option<prost_types::Any> = None;

        for page in &book.pages {
            let Some(event) = page.event.as_ref() else {
                continue;
            };

            if let Some(handler) = self.find_handler(&event.type_url) {
                if let Some(data) = handler(event, meta.root, meta.correlation_id, mode)? {
                    projection_data = Some(data);
                }
            }
        }

        Ok(angzarr::proto::Projection {
            cover: book.cover.clone(),
            projector: self.name.to_string(),
            sequence: next_sequence(Some(book)),
            projection: projection_data,
        })
    }
}

// ============================================================================
// Router<PmEventHandler> — PM dispatch (multi-domain)
// ============================================================================

/// Context passed to PM handlers.
pub struct PmContext<'a> {
    pub root: Option<&'a ProtoUuid>,
    pub correlation_id: &'a str,
    pub pm_state: Option<&'a EventBook>,
    pub destinations: &'a [EventBook],
}

/// Result from PM handler.
pub struct PmHandlerResult {
    pub commands: Vec<CommandBook>,
    pub pm_events: Option<EventBook>,
}

impl PmHandlerResult {
    pub fn empty() -> Self {
        Self { commands: vec![], pm_events: None }
    }

    pub fn commands(commands: Vec<CommandBook>) -> Self {
        Self { commands, pm_events: None }
    }

    pub fn state(pm_events: EventBook) -> Self {
        Self { commands: vec![], pm_events: Some(pm_events) }
    }

    pub fn both(commands: Vec<CommandBook>, pm_events: EventBook) -> Self {
        Self { commands, pm_events: Some(pm_events) }
    }
}

/// Handler for PM events.
pub type PmEventHandler = fn(&prost_types::Any, &PmContext) -> PmHandlerResult;

impl Router<PmEventHandler> {
    /// Dispatch trigger event to handler.
    pub fn dispatch(
        &self,
        trigger: &EventBook,
        pm_state: Option<&EventBook>,
        destinations: &[EventBook],
    ) -> PmHandlerResult {
        let meta = event_book_metadata(trigger);
        let ctx = PmContext {
            root: meta.root,
            correlation_id: meta.correlation_id,
            pm_state,
            destinations,
        };

        for page in &trigger.pages {
            let Some(event) = page.event.as_ref() else {
                continue;
            };

            if let Some(handler) = self.find_handler(&event.type_url) {
                return handler(event, &ctx);
            }
        }

        PmHandlerResult::empty()
    }
}

// ============================================================================
// Aggregate<S> — aggregate handler (Router + state rebuild)
// ============================================================================

/// Handler for aggregate commands.
pub type CommandHandler<S> = fn(&CommandBook, &[u8], &S, u32) -> Result<EventBook>;

/// Aggregate handler: command dispatch + event-sourced state rebuild.
pub struct Aggregate<S> {
    inner: Router<CommandHandler<S>>,
    rebuild: fn(Option<&EventBook>) -> S,
}

impl<S> Aggregate<S> {
    /// Create an aggregate handler.
    pub fn new(domain: &'static str, rebuild: fn(Option<&EventBook>) -> S) -> Self {
        Self {
            inner: Router::new(domain, AGGREGATE)
                .with(Dispatcher::new(domain)),
            rebuild,
        }
    }

    /// Register a handler for a command type suffix.
    pub fn on(mut self, type_suffix: &'static str, handler: CommandHandler<S>) -> Self {
        if let Some(dispatcher) = self.inner.dispatchers.pop() {
            self.inner.dispatchers.push(dispatcher.on(type_suffix, handler));
        }
        self
    }

    /// Get the domain.
    pub fn domain(&self) -> &'static str {
        self.inner.input_domain()
    }

    /// Build ComponentDescriptor.
    pub fn descriptor(&self) -> ComponentDescriptor {
        self.inner.descriptor()
    }

    /// Dispatch command to handler.
    #[allow(clippy::result_large_err)]
    pub fn dispatch(&self, cmd: ContextualCommand) -> std::result::Result<BusinessResponse, Status> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        let state = (self.rebuild)(prior_events);
        let next_seq = next_sequence(prior_events);

        let Some(cb) = command_book else {
            return Err(BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()).into());
        };

        let command_any = extract_command(cb)?;

        if let Some(handler) = self.inner.find_handler(&command_any.type_url) {
            let events = handler(cb, &command_any.value, &state, next_seq)?;
            return Ok(BusinessResponse {
                result: Some(business_response::Result::Events(events)),
            });
        }

        Err(
            BusinessError::Rejected(format!("{}: {}", errmsg::UNKNOWN_COMMAND, command_any.type_url))
                .into(),
        )
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{CommandPage, Cover, EventPage, event_page::Sequence};

    // --------------------------------------------------------------------
    // Aggregate tests
    // --------------------------------------------------------------------

    fn dummy_rebuild(_: Option<&EventBook>) -> String {
        "state".to_string()
    }

    fn cmd_handler_a(
        _cb: &CommandBook,
        _data: &[u8],
        _state: &String,
        seq: u32,
    ) -> Result<EventBook> {
        Ok(EventBook {
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(seq)),
                event: Some(prost_types::Any {
                    type_url: "HandledA".to_string(),
                    value: vec![],
                }),
                created_at: None,
            }],
            ..Default::default()
        })
    }

    fn cmd_handler_b(
        _cb: &CommandBook,
        _data: &[u8],
        _state: &String,
        _seq: u32,
    ) -> Result<EventBook> {
        Ok(EventBook::default())
    }

    #[test]
    fn test_aggregate_dispatches() {
        let agg = Aggregate::new("test", dummy_rebuild)
            .on("CommandA", cmd_handler_a)
            .on("CommandB", cmd_handler_b);

        let cmd = ContextualCommand {
            command: Some(CommandBook {
                cover: Some(Cover {
                    domain: "test".to_string(),
                    ..Default::default()
                }),
                pages: vec![CommandPage {
                    command: Some(prost_types::Any {
                        type_url: "type.test/CommandA".to_string(),
                        value: vec![],
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            events: None,
        };

        let result = agg.dispatch(cmd).unwrap();
        let events = match result.result.unwrap() {
            business_response::Result::Events(e) => e,
            _ => panic!("expected events"),
        };
        assert_eq!(events.pages.len(), 1);
    }

    #[test]
    fn test_aggregate_descriptor() {
        let agg = Aggregate::new("order", dummy_rebuild)
            .on("CreateOrder", cmd_handler_a)
            .on("CancelOrder", cmd_handler_b);

        let desc = agg.descriptor();
        assert_eq!(desc.name, "order");
        assert_eq!(desc.component_type, AGGREGATE);
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.inputs[0].domain, "order");
        assert_eq!(desc.inputs[0].types, vec!["CreateOrder", "CancelOrder"]);
    }

    // --------------------------------------------------------------------
    // Router<SagaEventHandler> tests
    // --------------------------------------------------------------------

    fn saga_handler(
        _event: &prost_types::Any,
        _root: Option<&ProtoUuid>,
        _corr_id: &str,
    ) -> Vec<CommandBook> {
        vec![CommandBook::default()]
    }

    #[test]
    fn test_saga_router_dispatches() {
        let router: Router<SagaEventHandler> = Router::new("test-saga", SAGA)
            .with(Dispatcher::new("order").on("OrderCompleted", saga_handler));

        let book = EventBook {
            cover: Some(Cover {
                domain: "order".to_string(),
                correlation_id: "corr-1".to_string(),
                ..Default::default()
            }),
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(1)),
                event: Some(prost_types::Any {
                    type_url: "type.test/OrderCompleted".to_string(),
                    value: vec![],
                }),
                created_at: None,
            }],
            ..Default::default()
        };

        let commands = router.dispatch(&book);
        assert_eq!(commands.len(), 1);
    }

    #[test]
    fn test_saga_router_descriptor() {
        let router: Router<SagaEventHandler> = Router::new("sag-order-fulfillment", SAGA)
            .with(Dispatcher::new("order").on("OrderCompleted", saga_handler));

        let desc = router.descriptor();
        assert_eq!(desc.name, "sag-order-fulfillment");
        assert_eq!(desc.component_type, SAGA);
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.inputs[0].domain, "order");
        assert_eq!(desc.inputs[0].types, vec!["OrderCompleted"]);
    }

    // --------------------------------------------------------------------
    // Router<ProjectorEventHandler> tests
    // --------------------------------------------------------------------

    fn projector_handler(
        _event: &prost_types::Any,
        _root: Option<&ProtoUuid>,
        _corr_id: &str,
        _mode: ProjectionMode,
    ) -> Result<Option<prost_types::Any>> {
        Ok(Some(prost_types::Any {
            type_url: "ProjectionData".to_string(),
            value: vec![42],
        }))
    }

    #[test]
    fn test_projector_router_dispatches() {
        let router: Router<ProjectorEventHandler> = Router::new("projector-test", PROJECTOR)
            .with(Dispatcher::new("inventory").on("StockReserved", projector_handler));

        let book = EventBook {
            cover: Some(Cover {
                domain: "inventory".to_string(),
                correlation_id: "corr-1".to_string(),
                ..Default::default()
            }),
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(1)),
                event: Some(prost_types::Any {
                    type_url: "type.test/StockReserved".to_string(),
                    value: vec![],
                }),
                created_at: None,
            }],
            ..Default::default()
        };

        let projection = router.dispatch(&book, ProjectionMode::Execute).unwrap();
        assert_eq!(projection.projector, "projector-test");
        assert!(projection.projection.is_some());
    }

    #[test]
    fn test_projector_router_descriptor() {
        let router: Router<ProjectorEventHandler> = Router::new("projector-inventory-stock", PROJECTOR)
            .with(Dispatcher::new("inventory").on("StockReserved", projector_handler));

        let desc = router.descriptor();
        assert_eq!(desc.name, "projector-inventory-stock");
        assert_eq!(desc.component_type, PROJECTOR);
        assert_eq!(desc.inputs[0].domain, "inventory");
    }

    // --------------------------------------------------------------------
    // Router<PmEventHandler> tests
    // --------------------------------------------------------------------

    fn pm_handler(_event: &prost_types::Any, _ctx: &PmContext) -> PmHandlerResult {
        PmHandlerResult::commands(vec![CommandBook::default()])
    }

    #[test]
    fn test_pm_router_dispatches() {
        let router = Router::new("order-fulfillment", PROCESS_MANAGER)
            .state_domain("order-fulfillment")
            .with(Dispatcher::new("order").on("OrderCreated", pm_handler as PmEventHandler))
            .with(Dispatcher::new("inventory").on("StockReserved", pm_handler as PmEventHandler));

        let trigger = EventBook {
            cover: Some(Cover {
                domain: "order".to_string(),
                correlation_id: "corr-1".to_string(),
                ..Default::default()
            }),
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(1)),
                event: Some(prost_types::Any {
                    type_url: "type.test/OrderCreated".to_string(),
                    value: vec![],
                }),
                created_at: None,
            }],
            ..Default::default()
        };

        let result = router.dispatch(&trigger, None, &[]);
        assert_eq!(result.commands.len(), 1);
    }

    #[test]
    fn test_pm_router_descriptor() {
        let router = Router::new("order-fulfillment", PROCESS_MANAGER)
            .state_domain("order-fulfillment")
            .with(Dispatcher::new("order").on("OrderCreated", pm_handler as PmEventHandler))
            .with(Dispatcher::new("inventory").on("StockReserved", pm_handler as PmEventHandler));

        let desc = router.descriptor();
        assert_eq!(desc.name, "order-fulfillment");
        assert_eq!(desc.component_type, PROCESS_MANAGER);
        assert_eq!(desc.inputs.len(), 2);

        let domains: Vec<_> = desc.inputs.iter().map(|t| t.domain.as_str()).collect();
        assert!(domains.contains(&"order"));
        assert!(domains.contains(&"inventory"));

        assert_eq!(router.get_state_domain(), Some("order-fulfillment"));
    }

    #[test]
    fn test_pm_handler_result_constructors() {
        let empty = PmHandlerResult::empty();
        assert!(empty.commands.is_empty());
        assert!(empty.pm_events.is_none());

        let cmds = PmHandlerResult::commands(vec![CommandBook::default()]);
        assert_eq!(cmds.commands.len(), 1);

        let state = PmHandlerResult::state(EventBook::default());
        assert!(state.pm_events.is_some());

        let both = PmHandlerResult::both(vec![CommandBook::default()], EventBook::default());
        assert_eq!(both.commands.len(), 1);
        assert!(both.pm_events.is_some());
    }
}
