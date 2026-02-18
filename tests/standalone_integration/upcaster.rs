//! Integration tests for the upcaster client component.
//!
//! Tests the full flow: events stored as V1 -> upcaster transforms to V2 -> business logic receives V2.
//! The upcaster is implemented by the client binary (same server as aggregate logic).

use crate::common::*;
use angzarr::proto::event_page::Sequence;
use angzarr::proto::upcaster_service_server::{UpcasterService, UpcasterServiceServer};
use angzarr::proto::{EventPage, UpcastRequest, UpcastResponse};
use angzarr::services::upcaster::Upcaster;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

// ============================================================================
// Test Upcaster Service
// ============================================================================

/// A test upcaster that transforms events from V1 to V2 format.
///
/// Simulates a real upcaster sidecar that would be deployed alongside an aggregate.
struct TestUpcasterService {
    call_count: AtomicU32,
    transformations: AtomicU32,
}

impl TestUpcasterService {
    fn new() -> Self {
        Self {
            call_count: AtomicU32::new(0),
            transformations: AtomicU32::new(0),
        }
    }

    fn calls(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }

    fn transformations(&self) -> u32 {
        self.transformations.load(Ordering::SeqCst)
    }
}

#[tonic::async_trait]
impl UpcasterService for TestUpcasterService {
    async fn upcast(
        &self,
        request: Request<UpcastRequest>,
    ) -> Result<Response<UpcastResponse>, Status> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        let req = request.into_inner();

        let transformed: Vec<EventPage> = req
            .events
            .into_iter()
            .map(|mut page| {
                if let Some(ref mut event) = page.event {
                    // Transform OrderCreatedV1 -> OrderCreatedV2
                    if event.type_url.contains("OrderCreatedV1") {
                        event.type_url = event.type_url.replace("V1", "V2");
                        // V2 adds a currency field (simulated by appending to value)
                        event.value.extend_from_slice(b"USD");
                        self.transformations.fetch_add(1, Ordering::SeqCst);
                    }
                    // Transform CustomerCreatedV1 -> CustomerCreatedV2
                    else if event.type_url.contains("CustomerCreatedV1") {
                        event.type_url = event.type_url.replace("V1", "V2");
                        // V2 renames customerId to customer_id (simulated)
                        self.transformations.fetch_add(1, Ordering::SeqCst);
                    }
                    // Events already at V2 or later pass through unchanged
                }
                page
            })
            .collect();

        Ok(Response::new(UpcastResponse {
            events: transformed,
        }))
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

/// Start a test upcaster service and return its address and a handle to check stats.
async fn start_upcaster_service() -> (String, Arc<TestUpcasterService>) {
    let service = Arc::new(TestUpcasterService::new());
    let service_clone = service.clone();

    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        Server::builder()
            .add_service(UpcasterServiceServer::from_arc(service_clone))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    (local_addr.to_string(), service)
}

fn make_v1_event(seq: u32, type_url: &str, value: Vec<u8>) -> EventPage {
    EventPage {
        sequence: Some(Sequence::Num(seq)),
        created_at: None,
        event: Some(prost_types::Any {
            type_url: type_url.to_string(),
            value,
        }),
        external_payload: None,
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

/// Test that the upcaster client correctly connects and transforms events.
#[tokio::test]
async fn test_upcaster_integration_transforms_v1_to_v2() {
    let (addr, service) = start_upcaster_service().await;

    let upcaster = Upcaster::from_address(&addr)
        .await
        .expect("Failed to connect");

    // Simulate loading V1 events from storage
    let stored_events = vec![
        make_v1_event(0, "example.OrderCreatedV1", vec![1, 2, 3]),
        make_v1_event(1, "example.OrderUpdated", vec![4, 5, 6]), // Already current version
        make_v1_event(2, "example.CustomerCreatedV1", vec![7, 8, 9]),
    ];

    let result = upcaster.upcast("order", stored_events).await.unwrap();

    // Verify transformations
    assert_eq!(result.len(), 3);

    // Event 0: OrderCreatedV1 -> OrderCreatedV2 with USD suffix
    let event0 = result[0].event.as_ref().unwrap();
    assert_eq!(event0.type_url, "example.OrderCreatedV2");
    assert_eq!(event0.value, vec![1, 2, 3, b'U', b'S', b'D']);

    // Event 1: Already current, unchanged type
    let event1 = result[1].event.as_ref().unwrap();
    assert_eq!(event1.type_url, "example.OrderUpdated");
    assert_eq!(event1.value, vec![4, 5, 6]);

    // Event 2: CustomerCreatedV1 -> CustomerCreatedV2
    let event2 = result[2].event.as_ref().unwrap();
    assert_eq!(event2.type_url, "example.CustomerCreatedV2");

    // Verify service was called and tracked transformations
    assert_eq!(service.calls(), 1);
    assert_eq!(service.transformations(), 2); // 2 V1 events transformed
}

/// Test that the upcaster preserves event ordering and sequence numbers.
#[tokio::test]
async fn test_upcaster_integration_preserves_ordering() {
    let (addr, _service) = start_upcaster_service().await;

    let upcaster = Upcaster::from_address(&addr).await.unwrap();

    // Events with non-sequential sequence numbers (e.g., from a range query)
    let stored_events = vec![
        make_v1_event(10, "example.OrderCreatedV1", vec![]),
        make_v1_event(15, "example.OrderCreatedV1", vec![]),
        make_v1_event(20, "example.OrderCreatedV1", vec![]),
    ];

    let result = upcaster.upcast("order", stored_events).await.unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].sequence, Some(Sequence::Num(10)));
    assert_eq!(result[1].sequence, Some(Sequence::Num(15)));
    assert_eq!(result[2].sequence, Some(Sequence::Num(20)));
}

/// Test multiple sequential upcast calls (simulating multiple aggregate loads).
#[tokio::test]
async fn test_upcaster_integration_multiple_calls() {
    let (addr, service) = start_upcaster_service().await;

    let upcaster = Upcaster::from_address(&addr).await.unwrap();

    // First aggregate load
    let events1 = vec![make_v1_event(0, "example.OrderCreatedV1", vec![1])];
    let result1 = upcaster.upcast("order", events1).await.unwrap();
    assert_eq!(result1.len(), 1);

    // Second aggregate load
    let events2 = vec![
        make_v1_event(0, "example.CustomerCreatedV1", vec![2]),
        make_v1_event(1, "example.CustomerCreatedV1", vec![3]),
    ];
    let result2 = upcaster.upcast("customer", events2).await.unwrap();
    assert_eq!(result2.len(), 2);

    // Verify both calls were made
    assert_eq!(service.calls(), 2);
    assert_eq!(service.transformations(), 3); // 1 + 2
}

/// Test that empty event lists are handled efficiently (no server call).
#[tokio::test]
async fn test_upcaster_integration_empty_events_no_call() {
    let (addr, service) = start_upcaster_service().await;

    let upcaster = Upcaster::from_address(&addr).await.unwrap();

    // Empty events should short-circuit
    let result = upcaster.upcast("order", vec![]).await.unwrap();

    assert!(result.is_empty());
    assert_eq!(service.calls(), 0); // No server call for empty events
}

/// Test upcaster with events that have no payload (edge case).
#[tokio::test]
async fn test_upcaster_integration_events_without_payload() {
    let (addr, _service) = start_upcaster_service().await;

    let upcaster = Upcaster::from_address(&addr).await.unwrap();

    let events = vec![EventPage {
        sequence: Some(Sequence::Num(0)),
        created_at: None,
        event: None, // No payload
        external_payload: None,
    }];

    let result = upcaster.upcast("order", events).await.unwrap();

    assert_eq!(result.len(), 1);
    assert!(result[0].event.is_none());
}

/// Test that upcaster handles large batches correctly.
#[tokio::test]
async fn test_upcaster_integration_large_batch() {
    let (addr, service) = start_upcaster_service().await;

    let upcaster = Upcaster::from_address(&addr).await.unwrap();

    // Create 100 V1 events
    let events: Vec<EventPage> = (0..100)
        .map(|i| make_v1_event(i, "example.OrderCreatedV1", vec![i as u8]))
        .collect();

    let result = upcaster.upcast("order", events).await.unwrap();

    assert_eq!(result.len(), 100);
    assert_eq!(service.calls(), 1);
    assert_eq!(service.transformations(), 100);

    // Verify all were transformed
    for (i, page) in result.iter().enumerate() {
        let event = page.event.as_ref().unwrap();
        assert_eq!(event.type_url, "example.OrderCreatedV2");
        assert_eq!(page.sequence, Some(Sequence::Num(i as u32)));
    }
}
