//! Tests for CommandDispatcher routing logic.
//!
//! CommandDispatcher is a thin routing layer that dispatches commands to
//! per-domain AggregateCommandHandlers. These tests verify:
//!
//! - Commands are routed to the correct domain handler
//! - Unknown domains return NotFound errors
//! - Compensation commands produce BusinessResponse wrappers
//!
//! The dispatcher has no business logic - just lookup and delegation.
//! If routing fails, sagas and process managers cannot execute cross-domain
//! workflows.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tonic::Status;
use uuid::Uuid;

use super::*;
use crate::handlers::core::AggregateCommandHandler;
use crate::orchestration::aggregate::{
    AggregateContext, AggregateContextFactory, ClientLogic, TemporalQuery,
};
use crate::proto::{
    business_response, event_page, BusinessResponse, CommandBook, ContextualCommand, EventBook,
    EventPage, Projection,
};
use crate::test_utils::{make_command_book, make_cover};

// ============================================================================
// Mock Implementations
// ============================================================================
//
// These mocks provide minimal implementations of the aggregate handler
// infrastructure. They're intentionally simple - just enough to test routing.

struct MockAggregateContext {
    domain: String,
}

#[async_trait]
impl AggregateContext for MockAggregateContext {
    async fn load_prior_events(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _temporal: &TemporalQuery,
    ) -> Result<EventBook, Status> {
        Ok(EventBook {
            cover: Some(make_cover(&self.domain)),
            pages: vec![],
            ..Default::default()
        })
    }

    async fn persist_events(
        &self,
        _prior: &EventBook,
        received: &EventBook,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _correlation_id: &str,
    ) -> Result<EventBook, Status> {
        Ok(received.clone())
    }

    async fn post_persist(&self, _events: &EventBook) -> Result<Vec<Projection>, Status> {
        Ok(vec![])
    }
}

struct MockClientLogic {
    events: EventBook,
}

impl MockClientLogic {
    fn new(domain: &str) -> Self {
        Self {
            events: EventBook {
                cover: Some(make_cover(domain)),
                pages: vec![EventPage {
                    sequence_type: Some(event_page::SequenceType::Sequence(1)),
                    created_at: None,
                    payload: Some(event_page::Payload::Event(prost_types::Any {
                        type_url: "test.Event".to_string(),
                        value: vec![1, 2, 3],
                    })),
                }],
                ..Default::default()
            },
        }
    }
}

#[async_trait]
impl ClientLogic for MockClientLogic {
    async fn invoke(&self, _cmd: ContextualCommand) -> Result<BusinessResponse, Status> {
        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(self.events.clone())),
        })
    }
}

struct MockContextFactory {
    domain: String,
    client_logic: Arc<dyn ClientLogic>,
}

impl MockContextFactory {
    fn new(domain: &str) -> Self {
        Self {
            domain: domain.to_string(),
            client_logic: Arc::new(MockClientLogic::new(domain)),
        }
    }
}

impl AggregateContextFactory for MockContextFactory {
    fn create(&self) -> Arc<dyn AggregateContext> {
        Arc::new(MockAggregateContext {
            domain: self.domain.clone(),
        })
    }

    fn domain(&self) -> &str {
        &self.domain
    }

    fn client_logic(&self) -> Arc<dyn ClientLogic> {
        self.client_logic.clone()
    }
}

fn create_test_handler(domain: &str) -> AggregateCommandHandler {
    AggregateCommandHandler::new(Arc::new(MockContextFactory::new(domain)))
}

/// Create a command book for a specific domain with a random root.
/// Thin wrapper for test readability - domain is the key routing parameter.
fn test_command_book(domain: &str) -> CommandBook {
    make_command_book(domain, Uuid::new_v4())
}

// ============================================================================
// CommandDispatcher::new Tests
// ============================================================================

/// Empty dispatcher is valid - useful for testing/initialization.
#[test]
fn test_new_with_empty_handlers() {
    let handlers = HashMap::new();
    let dispatcher = CommandDispatcher::new(handlers);
    assert!(dispatcher.domains().is_empty());
}

/// Single domain registration works.
#[test]
fn test_new_creates_dispatcher_with_handlers() {
    let mut handlers = HashMap::new();
    handlers.insert(
        "orders".to_string(),
        Arc::new(create_test_handler("orders")),
    );

    let dispatcher = CommandDispatcher::new(handlers);
    assert_eq!(dispatcher.domains().len(), 1);
}

/// Multiple domains can coexist - essential for multi-domain systems.
#[test]
fn test_new_with_multiple_handlers() {
    let mut handlers = HashMap::new();
    handlers.insert(
        "orders".to_string(),
        Arc::new(create_test_handler("orders")),
    );
    handlers.insert(
        "inventory".to_string(),
        Arc::new(create_test_handler("inventory")),
    );

    let dispatcher = CommandDispatcher::new(handlers);
    assert_eq!(dispatcher.domains().len(), 2);
}

// ============================================================================
// CommandDispatcher::domains Tests
// ============================================================================

/// Domains are correctly enumerable for introspection/debugging.
#[test]
fn test_domains_returns_handler_domains() {
    let mut handlers = HashMap::new();
    handlers.insert(
        "orders".to_string(),
        Arc::new(create_test_handler("orders")),
    );
    handlers.insert(
        "fulfillment".to_string(),
        Arc::new(create_test_handler("fulfillment")),
    );

    let dispatcher = CommandDispatcher::new(handlers);
    let domains = dispatcher.domains();

    assert!(domains.contains(&"orders"));
    assert!(domains.contains(&"fulfillment"));
}

// ============================================================================
// CommandDispatcher::get_handler Tests
// ============================================================================

/// Known domain returns handler reference.
#[test]
fn test_get_handler_returns_handler_for_known_domain() {
    let mut handlers = HashMap::new();
    handlers.insert(
        "orders".to_string(),
        Arc::new(create_test_handler("orders")),
    );

    let dispatcher = CommandDispatcher::new(handlers);
    assert!(dispatcher.get_handler("orders").is_some());
}

/// Unknown domain returns None, allowing caller to handle gracefully.
#[test]
fn test_get_handler_returns_none_for_unknown_domain() {
    let mut handlers = HashMap::new();
    handlers.insert(
        "orders".to_string(),
        Arc::new(create_test_handler("orders")),
    );

    let dispatcher = CommandDispatcher::new(handlers);
    assert!(dispatcher.get_handler("unknown").is_none());
}

// ============================================================================
// CommandDispatcher::execute Tests
// ============================================================================

/// Commands are routed to the correct handler based on domain.
/// This is the core routing behavior that enables multi-domain systems.
#[tokio::test]
async fn test_execute_routes_to_correct_handler() {
    let mut handlers = HashMap::new();
    handlers.insert(
        "orders".to_string(),
        Arc::new(create_test_handler("orders")),
    );

    let dispatcher = CommandDispatcher::new(handlers);
    let command = test_command_book("orders");

    let result = dispatcher.execute(command).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.events.is_some());
}

/// Unknown domain returns NotFound with domain name in message.
/// Error message must include domain for debugging.
#[tokio::test]
async fn test_execute_returns_not_found_for_unknown_domain() {
    let handlers = HashMap::new();

    let dispatcher = CommandDispatcher::new(handlers);
    let command = test_command_book("unknown");

    let result = dispatcher.execute(command).await;

    assert!(result.is_err());
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::NotFound);
    assert!(status.message().contains("unknown"));
}

// ============================================================================
// CommandDispatcher::execute_compensation Tests
// ============================================================================

/// Compensation wraps events in BusinessResponse for saga/PM consumption.
#[tokio::test]
async fn test_execute_compensation_returns_business_response() {
    let mut handlers = HashMap::new();
    handlers.insert(
        "orders".to_string(),
        Arc::new(create_test_handler("orders")),
    );

    let dispatcher = CommandDispatcher::new(handlers);
    let command = test_command_book("orders");

    let result = dispatcher.execute_compensation(command).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.result.is_some());
    match response.result {
        Some(crate::proto::business_response::Result::Events(_)) => {}
        _ => panic!("Expected Events result"),
    }
}

/// Compensation propagates routing errors - saga/PM needs to handle failure.
#[tokio::test]
async fn test_execute_compensation_propagates_not_found_error() {
    let handlers = HashMap::new();

    let dispatcher = CommandDispatcher::new(handlers);
    let command = test_command_book("unknown");

    let result = dispatcher.execute_compensation(command).await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}

// ============================================================================
// Clone Tests
// ============================================================================

/// Dispatcher is clonable - necessary for sharing across async tasks.
#[test]
fn test_dispatcher_clone() {
    let mut handlers = HashMap::new();
    handlers.insert(
        "orders".to_string(),
        Arc::new(create_test_handler("orders")),
    );

    let dispatcher = CommandDispatcher::new(handlers);
    let cloned = dispatcher.clone();

    assert_eq!(dispatcher.domains().len(), cloned.domains().len());
}
