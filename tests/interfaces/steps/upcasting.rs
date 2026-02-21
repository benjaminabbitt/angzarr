//! Upcasting interface step definitions.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use angzarr::proto::event_page;
use angzarr::proto::upcaster_service_server::{UpcasterService, UpcasterServiceServer};
use angzarr::proto::{EventPage, UpcastRequest, UpcastResponse};
use angzarr::services::upcaster::Upcaster;
use cucumber::{given, then, when, World};
use tokio::sync::Mutex;
use tonic::transport::Server;
use tonic::{Request, Response};

/// Test context for Upcasting scenarios.
#[derive(World)]
#[world(init = Self::new)]
pub struct UpcasterWorld {
    upcaster: Option<Arc<Upcaster>>,
    stored_events: Vec<EventPage>,
    loaded_events: Vec<EventPage>,
    last_error: Option<String>,
    mock_server: Option<MockServerHandle>,
    invocation_count: Arc<AtomicU32>,
    received_domain: Arc<Mutex<Option<String>>>,
}

impl std::fmt::Debug for UpcasterWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpcasterWorld")
            .field("stored_events", &self.stored_events.len())
            .field("loaded_events", &self.loaded_events.len())
            .field("last_error", &self.last_error)
            .finish()
    }
}

struct MockServerHandle {
    _addr: SocketAddr,
}

impl UpcasterWorld {
    fn new() -> Self {
        Self {
            upcaster: None,
            stored_events: Vec::new(),
            loaded_events: Vec::new(),
            last_error: None,
            mock_server: None,
            invocation_count: Arc::new(AtomicU32::new(0)),
            received_domain: Arc::new(Mutex::new(None)),
        }
    }

    fn make_event_page(seq: u32, type_url: &str, value: Vec<u8>) -> EventPage {
        EventPage {
            sequence: seq,
            created_at: None,
            payload: Some(event_page::Payload::Event(prost_types::Any {
                type_url: type_url.to_string(),
                value,
            })),
        }
    }

    fn get_type_url(page: &EventPage) -> &str {
        match &page.payload {
            Some(event_page::Payload::Event(e)) => &e.type_url,
            _ => "",
        }
    }

    fn get_value(page: &EventPage) -> &[u8] {
        match &page.payload {
            Some(event_page::Payload::Event(e)) => &e.value,
            _ => &[],
        }
    }
}

// ==========================================================================
// Mock Upcaster Services
// ==========================================================================

/// Mock upcaster that transforms V1 events to V2.
struct TransformingUpcaster {
    invocation_count: Arc<AtomicU32>,
    received_domain: Arc<Mutex<Option<String>>>,
}

#[tonic::async_trait]
impl UpcasterService for TransformingUpcaster {
    async fn upcast(
        &self,
        request: Request<UpcastRequest>,
    ) -> Result<Response<UpcastResponse>, tonic::Status> {
        self.invocation_count.fetch_add(1, Ordering::SeqCst);

        let req = request.into_inner();
        *self.received_domain.lock().await = Some(req.domain.clone());

        let transformed: Vec<EventPage> = req
            .events
            .into_iter()
            .map(|mut page| {
                if let Some(event_page::Payload::Event(ref mut event)) = page.payload {
                    if event.type_url.ends_with("V1") {
                        event.type_url = event.type_url.replace("V1", "V2");
                    }
                    if !event.value.is_empty() {
                        event.value.push(0xFF); // Migration marker
                    }
                }
                page
            })
            .collect();

        Ok(Response::new(UpcastResponse {
            events: transformed,
        }))
    }
}

/// Mock upcaster that fails with a specified error.
struct FailingUpcaster {
    error_message: String,
}

#[tonic::async_trait]
impl UpcasterService for FailingUpcaster {
    async fn upcast(
        &self,
        _request: Request<UpcastRequest>,
    ) -> Result<Response<UpcastResponse>, tonic::Status> {
        Err(tonic::Status::internal(&self.error_message))
    }
}

/// Mock upcaster that passes through unknown types unchanged.
struct PassthroughUpcaster;

#[tonic::async_trait]
impl UpcasterService for PassthroughUpcaster {
    async fn upcast(
        &self,
        request: Request<UpcastRequest>,
    ) -> Result<Response<UpcastResponse>, tonic::Status> {
        // Pass through all events unchanged
        Ok(Response::new(UpcastResponse {
            events: request.into_inner().events,
        }))
    }
}

async fn start_mock_server<S: UpcasterService>(service: S) -> (SocketAddr, Arc<Upcaster>) {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        Server::builder()
            .add_service(UpcasterServiceServer::new(service))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let upcaster = Upcaster::from_address(&local_addr.to_string())
        .await
        .expect("Failed to connect to mock upcaster");

    (local_addr, Arc::new(upcaster))
}

// ==========================================================================
// Background
// ==========================================================================

#[given("an Upcaster test environment")]
async fn given_upcaster_environment(_world: &mut UpcasterWorld) {
    // Environment is initialized via World::new
}

// ==========================================================================
// Schema Evolution
// ==========================================================================

#[given("an upcaster that transforms V1 events to V2")]
async fn given_transforming_upcaster(world: &mut UpcasterWorld) {
    let service = TransformingUpcaster {
        invocation_count: Arc::clone(&world.invocation_count),
        received_domain: Arc::clone(&world.received_domain),
    };
    let (addr, upcaster) = start_mock_server(service).await;
    world.mock_server = Some(MockServerHandle { _addr: addr });
    world.upcaster = Some(upcaster);
}

#[given(expr = "events with type {string} are stored")]
async fn given_events_with_type(world: &mut UpcasterWorld, type_url: String) {
    world.stored_events = vec![
        UpcasterWorld::make_event_page(0, &type_url, vec![1, 2, 3]),
        UpcasterWorld::make_event_page(1, &type_url, vec![4, 5, 6]),
    ];
}

#[when("I load the events through the upcaster")]
async fn when_load_through_upcaster(world: &mut UpcasterWorld) {
    let upcaster = world.upcaster.as_ref().expect("Upcaster not configured");
    match upcaster.upcast("test", world.stored_events.clone()).await {
        Ok(events) => {
            world.loaded_events = events;
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.message().to_string());
        }
    }
}

#[then(expr = "the events should have type {string}")]
async fn then_events_have_type(world: &mut UpcasterWorld, expected_type: String) {
    for event in &world.loaded_events {
        let type_url = UpcasterWorld::get_type_url(event);
        assert_eq!(
            type_url, expected_type,
            "Expected type {}, got {}",
            expected_type, type_url
        );
    }
}

#[then("the events should have the migration marker")]
async fn then_events_have_marker(world: &mut UpcasterWorld) {
    for event in &world.loaded_events {
        let value = UpcasterWorld::get_value(event);
        assert!(
            value.last() == Some(&0xFF),
            "Expected migration marker 0xFF at end of value"
        );
    }
}

#[then(expr = "the events should still have type {string}")]
async fn then_events_still_have_type(world: &mut UpcasterWorld, expected_type: String) {
    for event in &world.loaded_events {
        let type_url = UpcasterWorld::get_type_url(event);
        assert_eq!(
            type_url, expected_type,
            "Expected type {}, got {}",
            expected_type, type_url
        );
    }
}

#[given("mixed version events are stored")]
async fn given_mixed_version_events(world: &mut UpcasterWorld) {
    world.stored_events = vec![
        UpcasterWorld::make_event_page(0, "OrderCreatedV1", vec![1]),
        UpcasterWorld::make_event_page(1, "OrderUpdatedV2", vec![2]),
        UpcasterWorld::make_event_page(2, "OrderCompletedV1", vec![3]),
    ];
}

#[then("the mixed events should all be V2")]
async fn then_mixed_events_all_v2(world: &mut UpcasterWorld) {
    let expected_types = ["OrderCreatedV2", "OrderUpdatedV2", "OrderCompletedV2"];

    assert_eq!(
        world.loaded_events.len(),
        expected_types.len(),
        "Event count mismatch"
    );

    for (i, event) in world.loaded_events.iter().enumerate() {
        let type_url = UpcasterWorld::get_type_url(event);
        assert_eq!(
            type_url, expected_types[i],
            "Event {} expected type {}, got {}",
            i, expected_types[i], type_url
        );
    }
}

// ==========================================================================
// Sequence Preservation
// ==========================================================================

#[given(expr = "events with sequences {int}, {int}, {int} are stored")]
async fn given_events_with_sequences(world: &mut UpcasterWorld, s1: u32, s2: u32, s3: u32) {
    world.stored_events = vec![
        UpcasterWorld::make_event_page(s1, "OrderCreatedV1", vec![1]),
        UpcasterWorld::make_event_page(s2, "OrderUpdatedV1", vec![2]),
        UpcasterWorld::make_event_page(s3, "OrderCompletedV1", vec![3]),
    ];
}

#[then(expr = "the events should have sequences {int}, {int}, {int}")]
async fn then_events_have_sequences(world: &mut UpcasterWorld, s1: u32, s2: u32, s3: u32) {
    assert_eq!(world.loaded_events.len(), 3, "Expected 3 events");
    assert_eq!(world.loaded_events[0].sequence, s1);
    assert_eq!(world.loaded_events[1].sequence, s2);
    assert_eq!(world.loaded_events[2].sequence, s3);
}

#[given(expr = "{int} sequential events are stored")]
async fn given_sequential_events(world: &mut UpcasterWorld, count: u32) {
    world.stored_events = (0..count)
        .map(|i| UpcasterWorld::make_event_page(i, "OrderEventV1", vec![i as u8]))
        .collect();
}

#[then("the events should be in sequence order")]
async fn then_events_in_sequence_order(world: &mut UpcasterWorld) {
    for (i, event) in world.loaded_events.iter().enumerate() {
        assert_eq!(
            event.sequence, i as u32,
            "Event {} has wrong sequence {}",
            i, event.sequence
        );
    }
}

// ==========================================================================
// Disabled Upcaster
// ==========================================================================

#[given("upcasting is disabled")]
async fn given_upcasting_disabled(world: &mut UpcasterWorld) {
    world.upcaster = Some(Arc::new(Upcaster::disabled()));
}

#[when("I load the events")]
async fn when_load_events(world: &mut UpcasterWorld) {
    let upcaster = world.upcaster.as_ref().expect("Upcaster not configured");
    match upcaster.upcast("test", world.stored_events.clone()).await {
        Ok(events) => {
            world.loaded_events = events;
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.message().to_string());
        }
    }
}

#[given("an upcaster that tracks invocations")]
async fn given_tracking_upcaster(world: &mut UpcasterWorld) {
    let service = TransformingUpcaster {
        invocation_count: Arc::clone(&world.invocation_count),
        received_domain: Arc::clone(&world.received_domain),
    };
    let (addr, upcaster) = start_mock_server(service).await;
    world.mock_server = Some(MockServerHandle { _addr: addr });
    world.upcaster = Some(upcaster);
}

#[given("no events are stored")]
async fn given_no_events(world: &mut UpcasterWorld) {
    world.stored_events = Vec::new();
}

#[then("the upcaster should not be invoked")]
async fn then_upcaster_not_invoked(world: &mut UpcasterWorld) {
    let count = world.invocation_count.load(Ordering::SeqCst);
    assert_eq!(count, 0, "Upcaster was invoked {} times", count);
}

// ==========================================================================
// Error Handling
// ==========================================================================

#[given(expr = "an upcaster that fails with {string}")]
async fn given_failing_upcaster(world: &mut UpcasterWorld, error_message: String) {
    let service = FailingUpcaster { error_message };
    let (addr, upcaster) = start_mock_server(service).await;
    world.mock_server = Some(MockServerHandle { _addr: addr });
    world.upcaster = Some(upcaster);
}

#[given("events are stored")]
async fn given_some_events(world: &mut UpcasterWorld) {
    world.stored_events = vec![UpcasterWorld::make_event_page(
        0,
        "SomeEvent",
        vec![1, 2, 3],
    )];
}

#[when("I try to load the events through the upcaster")]
async fn when_try_load_through_upcaster(world: &mut UpcasterWorld) {
    let upcaster = world.upcaster.as_ref().expect("Upcaster not configured");
    match upcaster.upcast("test", world.stored_events.clone()).await {
        Ok(events) => {
            world.loaded_events = events;
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.message().to_string());
        }
    }
}

#[then(expr = "the operation should fail with {string}")]
async fn then_operation_fails_with(world: &mut UpcasterWorld, expected_error: String) {
    let error = world
        .last_error
        .as_ref()
        .expect("Expected operation to fail");
    assert!(
        error.contains(&expected_error),
        "Expected error containing '{}', got '{}'",
        expected_error,
        error
    );
}

#[given("an upcaster that only handles known types")]
async fn given_passthrough_upcaster(world: &mut UpcasterWorld) {
    let (addr, upcaster) = start_mock_server(PassthroughUpcaster).await;
    world.mock_server = Some(MockServerHandle { _addr: addr });
    world.upcaster = Some(upcaster);
}

#[then("the events should pass through unchanged")]
async fn then_events_unchanged(world: &mut UpcasterWorld) {
    assert_eq!(
        world.loaded_events.len(),
        world.stored_events.len(),
        "Event count mismatch"
    );

    for (loaded, stored) in world.loaded_events.iter().zip(&world.stored_events) {
        let loaded_type = UpcasterWorld::get_type_url(loaded);
        let stored_type = UpcasterWorld::get_type_url(stored);
        assert_eq!(
            loaded_type, stored_type,
            "Type changed unexpectedly: {} -> {}",
            stored_type, loaded_type
        );
    }
}

// ==========================================================================
// Batch Processing
// ==========================================================================

#[given(expr = "{int} events are stored")]
async fn given_n_events(world: &mut UpcasterWorld, count: u32) {
    world.stored_events = (0..count)
        .map(|i| UpcasterWorld::make_event_page(i, "BatchEventV1", vec![i as u8]))
        .collect();
}

#[then(expr = "all {int} events should be transformed")]
async fn then_all_events_transformed(world: &mut UpcasterWorld, count: u32) {
    assert_eq!(
        world.loaded_events.len(),
        count as usize,
        "Expected {} events, got {}",
        count,
        world.loaded_events.len()
    );

    for event in &world.loaded_events {
        let type_url = UpcasterWorld::get_type_url(event);
        assert!(
            type_url.ends_with("V2"),
            "Event type {} was not transformed",
            type_url
        );
    }
}

#[given("an upcaster that tracks domains")]
async fn given_domain_tracking_upcaster(world: &mut UpcasterWorld) {
    let service = TransformingUpcaster {
        invocation_count: Arc::clone(&world.invocation_count),
        received_domain: Arc::clone(&world.received_domain),
    };
    let (addr, upcaster) = start_mock_server(service).await;
    world.mock_server = Some(MockServerHandle { _addr: addr });
    world.upcaster = Some(upcaster);
}

#[given(expr = "events in domain {string} are stored")]
async fn given_events_in_domain(world: &mut UpcasterWorld, _domain: String) {
    world.stored_events = vec![UpcasterWorld::make_event_page(
        0,
        "DomainEventV1",
        vec![1, 2, 3],
    )];
}

#[when(expr = "I load the events for domain {string}")]
async fn when_load_events_for_domain(world: &mut UpcasterWorld, domain: String) {
    let upcaster = world.upcaster.as_ref().expect("Upcaster not configured");
    match upcaster.upcast(&domain, world.stored_events.clone()).await {
        Ok(events) => {
            world.loaded_events = events;
            world.last_error = None;
        }
        Err(e) => {
            world.last_error = Some(e.message().to_string());
        }
    }
}

#[then(expr = "the upcaster should receive domain {string}")]
async fn then_upcaster_received_domain(world: &mut UpcasterWorld, expected_domain: String) {
    let received = world.received_domain.lock().await;
    let domain = received.as_ref().expect("No domain received");
    assert_eq!(
        domain, &expected_domain,
        "Expected domain '{}', got '{}'",
        expected_domain, domain
    );
}
