//! Tests for EventBook gap-filling via EventQuery gRPC service.
//!
//! Tests the gRPC-based event fetching flow used by RemoteEventSource.
//! These complement the unit tests in gap_fill/filler.test.rs which use
//! LocalEventSource with mock stores.

use crate::common::*;
use angzarr::proto::event_query_service_server::EventQueryServiceServer;
use angzarr::services::gap_fill::{GapFiller, NoOpPositionStore, RemoteEventSource};
use angzarr::services::EventQueryService;
use angzarr::storage::mock::{MockEventStore, MockSnapshotStore};
use angzarr::storage::EventStore;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tonic::transport::Server;

/// Start an EventQuery gRPC server with test data.
async fn start_event_query_server(
    event_store: Arc<MockEventStore>,
    snapshot_store: Arc<MockSnapshotStore>,
) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let service = EventQueryService::new(event_store, snapshot_store);

    tokio::spawn(async move {
        Server::builder()
            .add_service(EventQueryServiceServer::new(service))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    addr
}

fn test_event(sequence: u32, event_type: &str) -> EventPage {
    EventPage {
        header: Some(PageHeader {
            sequence_type: Some(page_header::SequenceType::Sequence(sequence)),
        }),
        created_at: None,
        payload: Some(event_page::Payload::Event(Any {
            type_url: format!("type.googleapis.com/{}", event_type),
            value: vec![sequence as u8],
        })),
        committed: true,
        cascade_id: None,
    }
}

/// GapFiller fetches missing history via RemoteEventSource (gRPC).
///
/// When using NoOpPositionStore (no checkpoint), an EventBook starting
/// at sequence N>0 triggers fetching events 0..N to complete the history.
#[tokio::test]
async fn test_gap_filler_fetches_missing_history() {
    // Set up event store with full history
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());

    let domain = "orders";
    let root = Uuid::new_v4();

    // Store events 0-4
    let events: Vec<EventPage> = (0..5)
        .map(|i| test_event(i, &format!("Event{}", i)))
        .collect();
    event_store
        .add(domain, DEFAULT_EDITION, root, events, "", None, None)
        .await
        .unwrap();

    // Start EventQuery server
    let addr = start_event_query_server(event_store, snapshot_store).await;

    // Create GapFiller with RemoteEventSource
    let event_source = RemoteEventSource::connect(&addr.to_string())
        .await
        .expect("Failed to connect to EventQuery");
    let gap_filler = GapFiller::new(NoOpPositionStore, event_source);

    // Create incomplete EventBook (only event 4, missing 0-3)
    let incomplete = EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: Some(Edition {
                name: DEFAULT_EDITION.to_string(),
                divergences: vec![],
            }),
        }),
        pages: vec![test_event(4, "Event4")],
        snapshot: None,
        ..Default::default()
    };

    // Fill gaps
    let filled = gap_filler
        .fill_if_needed(incomplete)
        .await
        .expect("Fill failed");

    // Verify filled book has all events
    assert_eq!(filled.pages.len(), 5, "Should have all 5 events");

    // Verify sequence order
    for (i, page) in filled.pages.iter().enumerate() {
        assert_eq!(
            page.sequence_num() as usize,
            i,
            "Event {} should have sequence {}",
            i,
            i
        );
    }
}

/// GapFiller passes through already-complete books unchanged.
///
/// When an EventBook starts at sequence 0, no gap-filling is needed.
#[tokio::test]
async fn test_gap_filler_passes_through_complete_book() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());

    let addr = start_event_query_server(event_store, snapshot_store).await;

    let event_source = RemoteEventSource::connect(&addr.to_string())
        .await
        .expect("Failed to connect");
    let gap_filler = GapFiller::new(NoOpPositionStore, event_source);

    // Create complete EventBook (starts at sequence 0)
    let complete = EventBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: Some(Edition {
                name: DEFAULT_EDITION.to_string(),
                divergences: vec![],
            }),
        }),
        pages: vec![test_event(0, "Created"), test_event(1, "Updated")],
        snapshot: None,
        ..Default::default()
    };

    // Fill should return same book (no gaps to fill)
    let result = gap_filler
        .fill_if_needed(complete.clone())
        .await
        .expect("Fill failed");
    assert_eq!(result.pages.len(), 2, "Should pass through unchanged");
}

/// GapFiller prepends empty result for non-existent aggregate.
///
/// When fetching gap events for an aggregate that doesn't exist,
/// the result is the original pages (no prepended events).
#[tokio::test]
async fn test_gap_filler_handles_missing_aggregate() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());

    let addr = start_event_query_server(event_store, snapshot_store).await;

    let event_source = RemoteEventSource::connect(&addr.to_string())
        .await
        .expect("Failed to connect");
    let gap_filler = GapFiller::new(NoOpPositionStore, event_source);

    // Create book for non-existent aggregate
    let root = Uuid::new_v4();
    let incomplete = EventBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: Some(Edition {
                name: DEFAULT_EDITION.to_string(),
                divergences: vec![],
            }),
        }),
        pages: vec![test_event(5, "LateEvent")], // Missing 0-4
        snapshot: None,
        ..Default::default()
    };

    // Fill - should prepend nothing since aggregate doesn't exist
    // Result: 0 (gap events) + 1 (original) = 1 event
    let filled = gap_filler
        .fill_if_needed(incomplete)
        .await
        .expect("Fill failed");

    // Original event is still there, but gap events are empty (aggregate doesn't exist)
    assert_eq!(filled.pages.len(), 1, "Should have original event only");
    assert_eq!(filled.pages[0].sequence_num(), 5);
}

/// Service discovery resolves EventQuery via environment variable.
///
/// Tests that RemoteEventSource works with a client obtained from
/// service discovery via the EVENT_QUERY_ADDRESS env var fallback.
#[tokio::test]
async fn test_discovery_resolves_event_query_via_env_var() {
    use angzarr::discovery::{ServiceDiscovery, StaticServiceDiscovery};

    // Set up event store with full history
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());

    let domain = "orders";
    let root = Uuid::new_v4();

    // Store events 0-2
    let events: Vec<EventPage> = (0..3)
        .map(|i| test_event(i, &format!("Event{}", i)))
        .collect();
    event_store
        .add(domain, DEFAULT_EDITION, root, events, "", None, None)
        .await
        .unwrap();

    // Start EventQuery server
    let addr = start_event_query_server(event_store, snapshot_store).await;

    // Set env var for discovery fallback
    std::env::set_var("EVENT_QUERY_ADDRESS", addr.to_string());

    // Create static discovery (no K8s, will use env var fallback)
    let discovery = StaticServiceDiscovery::new();

    // Resolve EventQuery for domain - should use env var
    let eq_client = discovery
        .get_event_query(domain)
        .await
        .expect("Should resolve via EVENT_QUERY_ADDRESS");

    // Create GapFiller with the discovered client
    let event_source = RemoteEventSource::new(eq_client);
    let gap_filler = GapFiller::new(NoOpPositionStore, event_source);

    // Create incomplete EventBook (only event 2, missing 0-1)
    let incomplete = EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: Some(Edition {
                name: DEFAULT_EDITION.to_string(),
                divergences: vec![],
            }),
        }),
        pages: vec![test_event(2, "Event2")],
        snapshot: None,
        ..Default::default()
    };

    // Fill gaps via the client we got from discovery
    let filled = gap_filler
        .fill_if_needed(incomplete)
        .await
        .expect("Fill failed");

    assert_eq!(filled.pages.len(), 3, "Should have all 3 events after fill");

    // Clean up env var
    std::env::remove_var("EVENT_QUERY_ADDRESS");
}

/// Service discovery resolves EventQuery via registered aggregate.
///
/// Tests that RemoteEventSource works with a client obtained from
/// service discovery via explicit aggregate registration.
#[tokio::test]
async fn test_discovery_resolves_registered_aggregate() {
    use angzarr::discovery::{ServiceDiscovery, StaticServiceDiscovery};

    // Set up event store with full history
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());

    let domain = "products";
    let root = Uuid::new_v4();

    // Store events 0-1
    let events: Vec<EventPage> = (0..2)
        .map(|i| test_event(i, &format!("ProductEvent{}", i)))
        .collect();
    event_store
        .add(domain, DEFAULT_EDITION, root, events, "", None, None)
        .await
        .unwrap();

    // Start EventQuery server
    let addr = start_event_query_server(event_store, snapshot_store).await;

    // Create discovery and register aggregate
    let discovery = StaticServiceDiscovery::new();
    discovery
        .register_aggregate(domain, &addr.ip().to_string(), addr.port())
        .await;

    // Resolve EventQuery - should use registered aggregate
    let eq_client = discovery
        .get_event_query(domain)
        .await
        .expect("Should resolve via registered aggregate");

    // Create GapFiller with the discovered client
    let event_source = RemoteEventSource::new(eq_client);
    let gap_filler = GapFiller::new(NoOpPositionStore, event_source);

    // Create incomplete EventBook (only event 1, missing 0)
    let incomplete = EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: Some(Edition {
                name: DEFAULT_EDITION.to_string(),
                divergences: vec![],
            }),
        }),
        pages: vec![test_event(1, "ProductEvent1")],
        snapshot: None,
        ..Default::default()
    };

    // Fill gaps via the client we got from discovery
    let filled = gap_filler
        .fill_if_needed(incomplete)
        .await
        .expect("Fill failed");

    assert_eq!(filled.pages.len(), 2, "Should have all 2 events after fill");
}
