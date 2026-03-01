//! Aggregate handler interface step definitions.
//!
//! Tests the AggregateCommandHandler contract including:
//! - Domain identity
//! - Command execution with events
//! - Sync projector integration
//! - Command bus transport (wrap/extract)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use angzarr::handlers::core::aggregate::{
    wrap_command_for_bus, AggregateCommandHandler, SyncProjectorEntry,
};
use angzarr::orchestration::aggregate::{
    AggregateContext, AggregateContextFactory, ClientLogic, TemporalQuery,
};
use angzarr::proto::{
    business_response, event_page, BusinessResponse, CommandBook, CommandResponse, Cover,
    EventBook, EventPage, Projection, Uuid as ProtoUuid,
};
use angzarr::standalone::{ProjectionMode, ProjectorHandler};
use async_trait::async_trait;
use cucumber::{given, then, when, World};
use prost::Message;
use prost_types::Any;
use tokio::sync::RwLock;
use tonic::Status;
use uuid::Uuid;

/// Test context for AggregateHandler scenarios.
#[derive(World, Default)]
#[world(init = Self::new)]
pub struct AggregateHandlerWorld {
    handlers: Vec<(String, AggregateCommandHandler)>,
    last_handler: Option<AggregateCommandHandler>,
    last_response: Option<CommandResponse>,
    last_error: Option<Status>,
    last_command: Option<CommandBook>,
    last_wrapped: Option<EventBook>,
    last_extracted: Option<Option<CommandBook>>,
    projector_called: Arc<AtomicBool>,
    produces_events: bool,
    event_type: Option<String>,
}

impl std::fmt::Debug for AggregateHandlerWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AggregateHandlerWorld")
            .field("handlers_count", &self.handlers.len())
            .field("last_response", &self.last_response)
            .field("last_error", &self.last_error)
            .finish()
    }
}

impl AggregateHandlerWorld {
    fn new() -> Self {
        Self {
            handlers: Vec::new(),
            last_handler: None,
            last_response: None,
            last_error: None,
            last_command: None,
            last_wrapped: None,
            last_extracted: None,
            projector_called: Arc::new(AtomicBool::new(false)),
            produces_events: true,
            event_type: None,
        }
    }
}

// ==========================================================================
// Mock Implementations
// ==========================================================================

fn make_cover(domain: &str) -> Cover {
    Cover {
        domain: domain.to_string(),
        root: Some(ProtoUuid {
            value: Uuid::new_v4().as_bytes().to_vec(),
        }),
        correlation_id: "test-correlation".to_string(),
        edition: None,
        external_id: String::new(),
    }
}

fn make_cover_with_correlation(domain: &str, correlation: &str) -> Cover {
    Cover {
        domain: domain.to_string(),
        root: Some(ProtoUuid {
            value: Uuid::new_v4().as_bytes().to_vec(),
        }),
        correlation_id: correlation.to_string(),
        edition: None,
        external_id: String::new(),
    }
}

fn make_command_book(domain: &str) -> CommandBook {
    CommandBook {
        cover: Some(make_cover(domain)),
        pages: vec![],
        saga_origin: None,
    }
}

fn make_command_book_with_correlation(domain: &str, correlation: &str) -> CommandBook {
    CommandBook {
        cover: Some(make_cover_with_correlation(domain, correlation)),
        pages: vec![],
        saga_origin: None,
    }
}

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
    response_events: Arc<RwLock<EventBook>>,
}

impl MockClientLogic {
    fn new(response_events: EventBook) -> Self {
        Self {
            response_events: Arc::new(RwLock::new(response_events)),
        }
    }

    fn with_events(produces_events: bool, event_type: Option<String>) -> Self {
        let events = if produces_events {
            let type_url = event_type
                .map(|t| format!("type.googleapis.com/test.{}", t))
                .unwrap_or_else(|| "type.googleapis.com/test.Event".to_string());

            EventBook {
                cover: Some(make_cover("test")),
                pages: vec![EventPage {
                    sequence_type: Some(event_page::SequenceType::Sequence(1)),
                    created_at: None,
                    payload: Some(event_page::Payload::Event(Any {
                        type_url,
                        value: vec![1, 2, 3],
                    })),
                }],
                ..Default::default()
            }
        } else {
            EventBook {
                cover: Some(make_cover("test")),
                pages: vec![],
                ..Default::default()
            }
        };
        Self::new(events)
    }
}

#[async_trait]
impl ClientLogic for MockClientLogic {
    async fn invoke(
        &self,
        _cmd: angzarr::proto::ContextualCommand,
    ) -> Result<BusinessResponse, Status> {
        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(
                self.response_events.read().await.clone(),
            )),
        })
    }
}

struct MockContextFactory {
    domain: String,
    client_logic: Arc<dyn ClientLogic>,
}

impl MockContextFactory {
    fn new(domain: &str, produces_events: bool, event_type: Option<String>) -> Self {
        Self {
            domain: domain.to_string(),
            client_logic: Arc::new(MockClientLogic::with_events(produces_events, event_type)),
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

struct TrackingProjector {
    called: Arc<AtomicBool>,
}

#[async_trait]
impl ProjectorHandler for TrackingProjector {
    async fn handle(
        &self,
        _events: &EventBook,
        _mode: ProjectionMode,
    ) -> Result<Projection, Status> {
        self.called.store(true, Ordering::SeqCst);
        Ok(Projection {
            projector: "test-projector".to_string(),
            sequence: 1,
            cover: None,
            projection: Some(Any {
                type_url: "test.Projection".to_string(),
                value: b"projection-data".to_vec(),
            }),
        })
    }
}

// ==========================================================================
// Given Steps
// ==========================================================================

#[given("an aggregate handler test environment")]
async fn given_test_environment(world: &mut AggregateHandlerWorld) {
    *world = AggregateHandlerWorld::new();
}

#[given(expr = "an aggregate handler for domain {string}")]
async fn given_handler_for_domain(world: &mut AggregateHandlerWorld, domain: String) {
    let factory = Arc::new(MockContextFactory::new(
        &domain,
        world.produces_events,
        None,
    ));
    let handler = AggregateCommandHandler::new(factory);
    world.last_handler = Some(handler);
}

#[given(expr = "aggregate handlers for domains {string}, {string}, {string}, {string}")]
async fn given_multiple_handlers(
    world: &mut AggregateHandlerWorld,
    d1: String,
    d2: String,
    d3: String,
    d4: String,
) {
    for domain in [d1, d2, d3, d4] {
        let factory = Arc::new(MockContextFactory::new(&domain, true, None));
        let handler = AggregateCommandHandler::new(factory);
        world.handlers.push((domain, handler));
    }
}

#[given("an aggregate handler that produces events")]
async fn given_handler_produces_events(world: &mut AggregateHandlerWorld) {
    world.produces_events = true;
    let factory = Arc::new(MockContextFactory::new("test", true, None));
    let handler = AggregateCommandHandler::new(factory);
    world.last_handler = Some(handler);
}

#[given(expr = "an aggregate handler that produces a {string} event")]
async fn given_handler_produces_typed_event(world: &mut AggregateHandlerWorld, event_type: String) {
    world.produces_events = true;
    world.event_type = Some(event_type.clone());
    let factory = Arc::new(MockContextFactory::new("test", true, Some(event_type)));
    let handler = AggregateCommandHandler::new(factory);
    world.last_handler = Some(handler);
}

#[given("an aggregate handler with a tracking projector")]
async fn given_handler_with_tracking_projector(world: &mut AggregateHandlerWorld) {
    world.projector_called = Arc::new(AtomicBool::new(false));
}

#[given("the handler produces events")]
async fn given_produces_events(world: &mut AggregateHandlerWorld) {
    world.produces_events = true;
    let factory = Arc::new(MockContextFactory::new("test", true, None));
    let handler =
        AggregateCommandHandler::new(factory).with_sync_projectors(vec![SyncProjectorEntry {
            name: "test-projector".to_string(),
            handler: Arc::new(TrackingProjector {
                called: world.projector_called.clone(),
            }),
        }]);
    world.last_handler = Some(handler);
}

#[given("the handler produces no events")]
async fn given_produces_no_events(world: &mut AggregateHandlerWorld) {
    world.produces_events = false;
    let factory = Arc::new(MockContextFactory::new("test", false, None));
    let handler =
        AggregateCommandHandler::new(factory).with_sync_projectors(vec![SyncProjectorEntry {
            name: "test-projector".to_string(),
            handler: Arc::new(TrackingProjector {
                called: world.projector_called.clone(),
            }),
        }]);
    world.last_handler = Some(handler);
}

#[given("an aggregate handler with a sync projector")]
async fn given_handler_with_sync_projector(world: &mut AggregateHandlerWorld) {
    world.projector_called = Arc::new(AtomicBool::new(false));
    world.produces_events = true;
    let factory = Arc::new(MockContextFactory::new("test", true, None));
    let handler =
        AggregateCommandHandler::new(factory).with_sync_projectors(vec![SyncProjectorEntry {
            name: "test-projector".to_string(),
            handler: Arc::new(TrackingProjector {
                called: world.projector_called.clone(),
            }),
        }]);
    world.last_handler = Some(handler);
}

#[given(expr = "a command for domain {string} with correlation {string}")]
async fn given_command_with_correlation(
    world: &mut AggregateHandlerWorld,
    domain: String,
    correlation: String,
) {
    world.last_command = Some(make_command_book_with_correlation(&domain, &correlation));
}

#[given(expr = "a command for domain {string}")]
async fn given_command_for_domain(world: &mut AggregateHandlerWorld, domain: String) {
    world.last_command = Some(make_command_book(&domain));
}

#[given("a command book")]
async fn given_command_book(world: &mut AggregateHandlerWorld) {
    world.last_command = Some(make_command_book("test"));
}

#[given("an event book with no pages")]
async fn given_event_book_no_pages(world: &mut AggregateHandlerWorld) {
    world.last_wrapped = Some(EventBook {
        cover: Some(make_cover("test")),
        pages: vec![],
        ..Default::default()
    });
}

#[given(expr = "an event book with type URL {string}")]
async fn given_event_book_with_type_url(world: &mut AggregateHandlerWorld, type_url: String) {
    world.last_wrapped = Some(EventBook {
        cover: Some(make_cover("test")),
        pages: vec![EventPage {
            sequence_type: Some(event_page::SequenceType::Sequence(0)),
            created_at: None,
            payload: Some(event_page::Payload::Event(Any {
                type_url,
                value: vec![],
            })),
        }],
        ..Default::default()
    });
}

#[given("an event book with missing payload")]
async fn given_event_book_missing_payload(world: &mut AggregateHandlerWorld) {
    world.last_wrapped = Some(EventBook {
        cover: Some(make_cover("test")),
        pages: vec![EventPage {
            sequence_type: Some(event_page::SequenceType::Sequence(0)),
            created_at: None,
            payload: None,
        }],
        ..Default::default()
    });
}

// ==========================================================================
// When Steps
// ==========================================================================

#[when("I execute a command")]
async fn when_execute_command(world: &mut AggregateHandlerWorld) {
    let handler = world.last_handler.as_ref().expect("No handler configured");
    let command = make_command_book(handler.domain());

    match handler.execute(command).await {
        Ok(response) => {
            world.last_response = Some(response);
            world.last_error = None;
        }
        Err(e) => {
            world.last_response = None;
            world.last_error = Some(e);
        }
    }
}

#[when("the command is wrapped for bus transport")]
async fn when_wrap_for_bus(world: &mut AggregateHandlerWorld) {
    let command = world.last_command.as_ref().expect("No command configured");
    world.last_wrapped = Some(wrap_command_for_bus(command));
}

#[when("the command is wrapped and then extracted")]
async fn when_wrap_and_extract(world: &mut AggregateHandlerWorld) {
    let command = world.last_command.as_ref().expect("No command configured");
    let wrapped = wrap_command_for_bus(command);
    world.last_extracted = Some(extract_command_from_event_book(&wrapped));
}

#[when("I try to extract a command")]
async fn when_try_extract(world: &mut AggregateHandlerWorld) {
    let book = world
        .last_wrapped
        .as_ref()
        .expect("No event book configured");
    world.last_extracted = Some(extract_command_from_event_book(book));
}

// Helper function to extract command (mirrors the private function in aggregate.rs)
fn extract_command_from_event_book(book: &EventBook) -> Option<CommandBook> {
    let page = book.pages.first()?;
    let event = match &page.payload {
        Some(event_page::Payload::Event(any)) => any,
        _ => return None,
    };
    if !event.type_url.ends_with("angzarr.CommandBook") {
        return None;
    }
    CommandBook::decode(event.value.as_slice()).ok()
}

// ==========================================================================
// Then Steps
// ==========================================================================

#[then(expr = "the handler domain should be {string}")]
async fn then_handler_domain_is(world: &mut AggregateHandlerWorld, expected: String) {
    let handler = world.last_handler.as_ref().expect("No handler configured");
    assert_eq!(
        handler.domain(),
        expected,
        "Handler domain mismatch: expected '{}', got '{}'",
        expected,
        handler.domain()
    );
}

#[then("the handler domain should not be empty")]
async fn then_handler_domain_not_empty(world: &mut AggregateHandlerWorld) {
    let handler = world.last_handler.as_ref().expect("No handler configured");
    assert!(
        !handler.domain().is_empty(),
        "Handler domain should not be empty"
    );
}

#[then("each handler should report its configured domain")]
async fn then_each_handler_reports_domain(world: &mut AggregateHandlerWorld) {
    for (expected_domain, handler) in &world.handlers {
        assert_eq!(
            handler.domain(),
            expected_domain,
            "Handler domain mismatch for '{}'",
            expected_domain
        );
    }
}

#[then("the response should contain events")]
async fn then_response_has_events(world: &mut AggregateHandlerWorld) {
    let response = world.last_response.as_ref().expect("No response");
    assert!(
        response.events.is_some(),
        "Response should contain events, but events is None"
    );
}

#[then("the response should have at least one event page")]
async fn then_response_has_event_pages(world: &mut AggregateHandlerWorld) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("Response has no events");
    assert!(
        !events.pages.is_empty(),
        "Response should have at least one event page"
    );
}

#[then("the events should include the produced event")]
async fn then_events_include_produced(world: &mut AggregateHandlerWorld) {
    let response = world.last_response.as_ref().expect("No response");
    let events = response.events.as_ref().expect("Response has no events");

    if let Some(ref event_type) = world.event_type {
        let expected_url = format!("type.googleapis.com/test.{}", event_type);
        let has_event = events.pages.iter().any(|p| {
            matches!(&p.payload, Some(event_page::Payload::Event(any)) if any.type_url == expected_url)
        });
        assert!(
            has_event,
            "Expected event type '{}' not found in response",
            event_type
        );
    }
}

#[then("the sync projector should have been called")]
async fn then_projector_called(world: &mut AggregateHandlerWorld) {
    assert!(
        world.projector_called.load(Ordering::SeqCst),
        "Sync projector should have been called when events are present"
    );
}

#[then("the sync projector should not have been called")]
async fn then_projector_not_called(world: &mut AggregateHandlerWorld) {
    assert!(
        !world.projector_called.load(Ordering::SeqCst),
        "Sync projector should NOT have been called when no events"
    );
}

#[then("the response should include projector output")]
async fn then_response_has_projector_output(world: &mut AggregateHandlerWorld) {
    let response = world.last_response.as_ref().expect("No response");
    assert!(
        !response.projections.is_empty(),
        "Response should include projector output"
    );
}

#[then(expr = "the wrapped event book should have domain {string}")]
async fn then_wrapped_has_domain(world: &mut AggregateHandlerWorld, expected: String) {
    let wrapped = world.last_wrapped.as_ref().expect("No wrapped event book");
    let domain = wrapped
        .cover
        .as_ref()
        .map(|c| c.domain.as_str())
        .unwrap_or("");
    assert_eq!(domain, expected, "Wrapped event book domain mismatch");
}

#[then(expr = "the wrapped event book should have correlation {string}")]
async fn then_wrapped_has_correlation(world: &mut AggregateHandlerWorld, expected: String) {
    let wrapped = world.last_wrapped.as_ref().expect("No wrapped event book");
    let correlation = wrapped
        .cover
        .as_ref()
        .map(|c| c.correlation_id.as_str())
        .unwrap_or("");
    assert_eq!(
        correlation, expected,
        "Wrapped event book correlation mismatch"
    );
}

#[then(expr = "the wrapped event book should have exactly {int} page")]
async fn then_wrapped_has_pages(world: &mut AggregateHandlerWorld, count: usize) {
    let wrapped = world.last_wrapped.as_ref().expect("No wrapped event book");
    assert_eq!(
        wrapped.pages.len(),
        count,
        "Expected {} pages, got {}",
        count,
        wrapped.pages.len()
    );
}

#[then(expr = "the wrapped page type URL should end with {string}")]
async fn then_wrapped_type_url_ends_with(world: &mut AggregateHandlerWorld, suffix: String) {
    let wrapped = world.last_wrapped.as_ref().expect("No wrapped event book");
    let page = wrapped.pages.first().expect("No pages in wrapped book");
    let type_url = match &page.payload {
        Some(event_page::Payload::Event(any)) => &any.type_url,
        _ => panic!("Expected Event payload"),
    };
    assert!(
        type_url.ends_with(&suffix),
        "Type URL '{}' should end with '{}'",
        type_url,
        suffix
    );
}

#[then(expr = "the extracted command should have domain {string}")]
async fn then_extracted_has_domain(world: &mut AggregateHandlerWorld, expected: String) {
    let extracted = world
        .last_extracted
        .as_ref()
        .expect("No extraction result")
        .as_ref()
        .expect("Extraction returned None");
    let domain = extracted
        .cover
        .as_ref()
        .map(|c| c.domain.as_str())
        .unwrap_or("");
    assert_eq!(domain, expected, "Extracted command domain mismatch");
}

#[then("extraction should return none")]
async fn then_extraction_returns_none(world: &mut AggregateHandlerWorld) {
    let extracted = world.last_extracted.as_ref().expect("No extraction result");
    assert!(
        extracted.is_none(),
        "Extraction should return None, got {:?}",
        extracted
    );
}
