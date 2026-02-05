//! DRY dispatch via router types.
//!
//! `CommandRouter<S>` replaces manual if/else chains in aggregate handlers.
//! `EventRouter` replaces manual if/else chains in saga event handlers.
//!
//! Both auto-derive `ComponentDescriptor` from their `.on()` registrations,
//! eliminating manual event type declarations.

use std::collections::HashMap;

use angzarr::proto::{
    business_response, BusinessResponse, CommandBook, ComponentDescriptor, ContextualCommand,
    EventBook, Target, Uuid as ProtoUuid,
};
use tonic::Status;

use crate::{errmsg, event_book_metadata, extract_command, next_sequence, BusinessError, Result};

// ============================================================================
// CommandRouter — aggregate dispatch
// ============================================================================

/// Handler function for a single command type.
///
/// Receives the CommandBook (for cover metadata), raw command bytes,
/// rebuilt state, and next sequence number. Returns new events.
pub type CommandHandler<S> = fn(&CommandBook, &[u8], &S, u32) -> Result<EventBook>;

/// DRY command dispatcher for aggregates.
///
/// Matches command type_url suffixes and dispatches to registered handler
/// functions. Auto-derives `ComponentDescriptor` from registrations.
///
/// # Example
///
/// ```ignore
/// let router = CommandRouter::new("order", rebuild_state)
///     .on("CreateOrder", handle_create_order)
///     .on("CancelOrder", handle_cancel_order);
///
/// // In AggregateLogic::handle:
/// router.dispatch(cmd)
///
/// // For topology:
/// router.descriptor()
/// ```
pub struct CommandRouter<S> {
    domain: &'static str,
    rebuild: fn(Option<&EventBook>) -> S,
    handlers: Vec<(&'static str, CommandHandler<S>)>,
}

impl<S> CommandRouter<S> {
    /// Create a new command router for a domain.
    ///
    /// - `domain`: The aggregate's domain name (e.g., "order").
    /// - `rebuild`: Function to rebuild state from prior events.
    pub fn new(domain: &'static str, rebuild: fn(Option<&EventBook>) -> S) -> Self {
        Self {
            domain,
            rebuild,
            handlers: Vec::new(),
        }
    }

    /// Register a handler for a command type_url suffix.
    ///
    /// The suffix is matched against the end of the command's type_url.
    /// E.g., `.on("CreateOrder", handle_create_order)` matches any
    /// type_url ending in "CreateOrder".
    pub fn on(mut self, type_suffix: &'static str, handler: CommandHandler<S>) -> Self {
        self.handlers.push((type_suffix, handler));
        self
    }

    /// Dispatch a contextual command to the appropriate handler.
    ///
    /// Extracts command + prior events, rebuilds state, matches type_url
    /// suffix, and calls the registered handler.
    #[allow(clippy::result_large_err)]
    pub fn dispatch(
        &self,
        cmd: ContextualCommand,
    ) -> std::result::Result<BusinessResponse, Status> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        let state = (self.rebuild)(prior_events);
        let next_seq = next_sequence(prior_events);

        let Some(cb) = command_book else {
            return Err(BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()).into());
        };

        let command_any = extract_command(cb)?;

        for (suffix, handler) in &self.handlers {
            if command_any.type_url.ends_with(suffix) {
                let events = handler(cb, &command_any.value, &state, next_seq)?;
                return Ok(BusinessResponse {
                    result: Some(business_response::Result::Events(events)),
                });
            }
        }

        Err(
            BusinessError::Rejected(format!("{}: {}", errmsg::UNKNOWN_COMMAND, command_any.type_url))
                .into(),
        )
    }

    /// Build a ComponentDescriptor from registered handlers.
    ///
    /// Returns a descriptor with auto-derived command type suffixes
    /// as the input types.
    pub fn descriptor(&self) -> ComponentDescriptor {
        ComponentDescriptor {
            name: self.domain.to_string(),
            component_type: "aggregate".to_string(),
            inputs: vec![Target {
                domain: self.domain.to_string(),
                types: self.handlers.iter().map(|(s, _)| (*s).to_string()).collect(),
            }],
            outputs: vec![], // Aggregates don't send commands to other domains
        }
    }

    /// Get the domain name.
    pub fn domain(&self) -> &'static str {
        self.domain
    }
}

// ============================================================================
// EventRouter — saga dispatch
// ============================================================================

/// Handler function for a single event type in a saga.
///
/// Receives the raw event (for type-specific decoding), source root UUID,
/// and correlation_id. Returns commands to execute.
pub type SagaEventHandler = fn(&prost_types::Any, Option<&ProtoUuid>, &str) -> Vec<CommandBook>;

/// DRY event dispatcher for sagas.
///
/// Matches event type_url suffixes and dispatches to registered handlers.
/// Auto-derives `ComponentDescriptor` from registrations.
///
/// # Example
///
/// ```ignore
/// let router = EventRouter::new("order-inventory", "order")
///     .on("OrderCreated", handle_order_created)
///     .sends("inventory", "ReserveStock");
///
/// // In SagaLogic::execute:
/// router.dispatch(source)
///
/// // For topology:
/// router.descriptor()
/// ```
pub struct EventRouter {
    name: &'static str,
    input_domain: &'static str,
    /// Maps output domain -> command types sent to that domain
    outputs: HashMap<&'static str, Vec<&'static str>>,
    handlers: Vec<(&'static str, SagaEventHandler)>,
}

impl EventRouter {
    /// Create a new event router.
    ///
    /// - `name`: The saga's name (e.g., "order-inventory").
    /// - `input_domain`: The domain to subscribe to for events.
    pub fn new(name: &'static str, input_domain: &'static str) -> Self {
        Self {
            name,
            input_domain,
            outputs: HashMap::new(),
            handlers: Vec::new(),
        }
    }

    /// Declare a command this saga sends to a target domain.
    ///
    /// Call multiple times to declare multiple commands to the same or different domains.
    pub fn sends(mut self, domain: &'static str, command_type: &'static str) -> Self {
        self.outputs
            .entry(domain)
            .or_insert_with(Vec::new)
            .push(command_type);
        self
    }

    /// Register a handler for an event type_url suffix.
    ///
    /// The suffix is matched against the end of the event's type_url.
    /// E.g., `.on("OrderCreated", handle_order_created)` matches any
    /// type_url ending in "OrderCreated".
    pub fn on(mut self, type_suffix: &'static str, handler: SagaEventHandler) -> Self {
        self.handlers.push((type_suffix, handler));
        self
    }

    /// Dispatch all events in an EventBook to registered handlers.
    ///
    /// Iterates pages, matches type_url suffixes, and collects commands.
    pub fn dispatch(&self, book: &EventBook) -> Vec<CommandBook> {
        let meta = event_book_metadata(book);
        book.pages
            .iter()
            .filter_map(|page| page.event.as_ref())
            .flat_map(|event| {
                for (suffix, handler) in &self.handlers {
                    if event.type_url.ends_with(suffix) {
                        return handler(event, meta.root, meta.correlation_id);
                    }
                }
                vec![]
            })
            .collect()
    }

    /// Build a ComponentDescriptor from registered handlers.
    ///
    /// Returns a descriptor with auto-derived event type suffixes
    /// as input types, plus declared output command types.
    pub fn descriptor(&self) -> ComponentDescriptor {
        ComponentDescriptor {
            name: self.name.to_string(),
            component_type: "saga".to_string(),
            inputs: vec![Target {
                domain: self.input_domain.to_string(),
                types: self.handlers.iter().map(|(s, _)| (*s).to_string()).collect(),
            }],
            outputs: self
                .outputs
                .iter()
                .map(|(domain, commands)| Target {
                    domain: (*domain).to_string(),
                    types: commands.iter().map(|c| (*c).to_string()).collect(),
                })
                .collect(),
        }
    }

    /// Get the input domain this router subscribes to.
    pub fn input_domain(&self) -> &'static str {
        self.input_domain
    }

    /// Get the declared output domains.
    pub fn output_domains(&self) -> Vec<&'static str> {
        self.outputs.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{CommandPage, Cover, EventPage, event_page::Sequence};

    fn dummy_rebuild(_: Option<&EventBook>) -> String {
        "state".to_string()
    }

    fn handler_a(
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

    fn handler_b(
        _cb: &CommandBook,
        _data: &[u8],
        _state: &String,
        _seq: u32,
    ) -> Result<EventBook> {
        Ok(EventBook::default())
    }

    #[test]
    fn test_command_router_dispatches_correct_handler() {
        let router = CommandRouter::new("test", dummy_rebuild)
            .on("CommandA", handler_a)
            .on("CommandB", handler_b);

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

        let result = router.dispatch(cmd).unwrap();
        let events = match result.result.unwrap() {
            business_response::Result::Events(e) => e,
            _ => panic!("expected events"),
        };
        assert_eq!(events.pages.len(), 1);
        assert_eq!(
            events.pages[0].event.as_ref().unwrap().type_url,
            "HandledA"
        );
    }

    #[test]
    fn test_command_router_unknown_command() {
        let router = CommandRouter::new("test", dummy_rebuild).on("CommandA", handler_a);

        let cmd = ContextualCommand {
            command: Some(CommandBook {
                cover: Some(Cover {
                    domain: "test".to_string(),
                    ..Default::default()
                }),
                pages: vec![CommandPage {
                    command: Some(prost_types::Any {
                        type_url: "type.test/UnknownCommand".to_string(),
                        value: vec![],
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            events: None,
        };

        let result = router.dispatch(cmd);
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert!(status.message().contains("Unknown command type"));
    }

    #[test]
    fn test_command_router_descriptor() {
        let router = CommandRouter::new("order", dummy_rebuild)
            .on("CreateOrder", handler_a)
            .on("CancelOrder", handler_b);

        let desc = router.descriptor();
        assert_eq!(desc.name, "order");
        assert_eq!(desc.component_type, "aggregate");
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.inputs[0].domain, "order");
        assert_eq!(desc.inputs[0].types, vec!["CreateOrder", "CancelOrder"]);
    }

    fn saga_handler(
        _event: &prost_types::Any,
        _root: Option<&ProtoUuid>,
        _corr_id: &str,
    ) -> Vec<CommandBook> {
        vec![CommandBook::default()]
    }

    #[test]
    fn test_event_router_dispatches() {
        let router = EventRouter::new("test-saga", "order")
            .sends("fulfillment", "Ship")
            .on("OrderCompleted", saga_handler);

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
    fn test_event_router_skips_unmatched() {
        let router = EventRouter::new("test-saga", "order")
            .on("OrderCompleted", saga_handler);

        let book = EventBook {
            cover: Some(Cover {
                domain: "order".to_string(),
                ..Default::default()
            }),
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(1)),
                event: Some(prost_types::Any {
                    type_url: "type.test/SomethingElse".to_string(),
                    value: vec![],
                }),
                created_at: None,
            }],
            ..Default::default()
        };

        let commands = router.dispatch(&book);
        assert!(commands.is_empty());
    }

    #[test]
    fn test_event_router_descriptor() {
        let router = EventRouter::new("order-fulfillment", "order")
            .sends("fulfillment", "Ship")
            .on("OrderCompleted", saga_handler);

        let desc = router.descriptor();
        assert_eq!(desc.name, "order-fulfillment");
        assert_eq!(desc.component_type, "saga");
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.inputs[0].domain, "order");
        assert_eq!(desc.inputs[0].types, vec!["OrderCompleted"]);
        assert_eq!(desc.outputs.len(), 1);
        assert_eq!(desc.outputs[0].domain, "fulfillment");
        assert_eq!(desc.outputs[0].types, vec!["Ship"]);
    }

    #[test]
    fn test_command_router_descriptor_has_empty_outputs() {
        let router = CommandRouter::new("order", dummy_rebuild)
            .on("CreateOrder", handler_a);

        let desc = router.descriptor();
        assert!(desc.outputs.is_empty());
    }
}
