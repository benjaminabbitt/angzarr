//! Tests for EventBook repair via EventQuery gRPC service.

use crate::common::*;
use angzarr::proto::event_query_service_server::EventQueryServiceServer;
use angzarr::services::event_book_repair::repair_if_needed;
use angzarr::services::{EventBookRepairer, EventQueryService};
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
        sequence,
        created_at: None,
        payload: Some(event_page::Payload::Event(Any {
            type_url: format!("type.googleapis.com/{}", event_type),
            value: vec![sequence as u8],
        })),
    }
}

#[tokio::test]
async fn test_repairer_fetches_missing_history() {
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
        .add(domain, DEFAULT_EDITION, root, events, "")
        .await
        .unwrap();

    // Start EventQuery server
    let addr = start_event_query_server(event_store, snapshot_store).await;

    // Create repairer
    let mut repairer = EventBookRepairer::connect(&addr.to_string())
        .await
        .expect("Failed to connect to EventQuery");

    // Create incomplete EventBook (only event 4, missing 0-3)
    let incomplete = EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![test_event(4, "Event4")],
        snapshot: None,
        ..Default::default()
    };

    // Verify it's incomplete
    assert!(!repairer.is_complete(&incomplete));

    // Repair it
    let repaired = repairer.repair(incomplete).await.expect("Repair failed");

    // Verify repaired book is complete with all events
    assert!(repairer.is_complete(&repaired));
    assert_eq!(repaired.pages.len(), 5, "Should have all 5 events");

    // Verify sequence order
    for (i, page) in repaired.pages.iter().enumerate() {
        assert_eq!(
            page.sequence as usize, i,
            "Event {} should have sequence {}",
            i, i
        );
    }
}

#[tokio::test]
async fn test_repairer_passes_through_complete_book() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());

    let addr = start_event_query_server(event_store, snapshot_store).await;

    let mut repairer = EventBookRepairer::connect(&addr.to_string())
        .await
        .expect("Failed to connect");

    // Create complete EventBook (starts at sequence 0)
    let complete = EventBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![test_event(0, "Created"), test_event(1, "Updated")],
        snapshot: None,
        ..Default::default()
    };

    // Verify it's already complete
    assert!(repairer.is_complete(&complete));

    // Repair should return same book
    let result = repairer
        .repair(complete.clone())
        .await
        .expect("Repair failed");
    assert_eq!(result.pages.len(), 2, "Should pass through unchanged");
}

#[tokio::test]
async fn test_repairer_handles_empty_aggregate() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());

    let addr = start_event_query_server(event_store, snapshot_store).await;

    let mut repairer = EventBookRepairer::connect(&addr.to_string())
        .await
        .expect("Failed to connect");

    // Create incomplete book for non-existent aggregate
    let root = Uuid::new_v4();
    let incomplete = EventBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![test_event(5, "LateEvent")], // Missing 0-4
        snapshot: None,
        ..Default::default()
    };

    // Repair - should return empty book since aggregate doesn't exist
    let repaired = repairer.repair(incomplete).await.expect("Repair failed");

    // Empty book is considered complete
    assert!(repairer.is_complete(&repaired));
    assert!(
        repaired.pages.is_empty(),
        "Should return empty for non-existent aggregate"
    );
}

#[tokio::test]
async fn test_discovery_resolves_event_query_via_env_var() {
    use angzarr::discovery::{K8sServiceDiscovery, ServiceDiscovery};

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
        .add(domain, DEFAULT_EDITION, root, events, "")
        .await
        .unwrap();

    // Start EventQuery server
    let addr = start_event_query_server(event_store, snapshot_store).await;

    // Set env var for discovery fallback
    std::env::set_var("EVENT_QUERY_ADDRESS", addr.to_string());

    // Create static discovery (no K8s, will use env var fallback)
    let discovery = K8sServiceDiscovery::new_static();

    // Resolve EventQuery for domain - should use env var
    let mut eq_client = discovery
        .get_event_query(domain)
        .await
        .expect("Should resolve via EVENT_QUERY_ADDRESS");

    // Create incomplete EventBook (only event 2, missing 0-1)
    let incomplete = EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![test_event(2, "Event2")],
        snapshot: None,
        ..Default::default()
    };

    // Repair via the client we got from discovery
    let repaired = repair_if_needed(&mut eq_client, incomplete)
        .await
        .expect("Repair failed");

    assert_eq!(
        repaired.pages.len(),
        3,
        "Should have all 3 events after repair"
    );

    // Clean up env var
    std::env::remove_var("EVENT_QUERY_ADDRESS");
}

#[tokio::test]
async fn test_discovery_resolves_registered_aggregate() {
    use angzarr::discovery::{K8sServiceDiscovery, ServiceDiscovery};

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
        .add(domain, DEFAULT_EDITION, root, events, "")
        .await
        .unwrap();

    // Start EventQuery server
    let addr = start_event_query_server(event_store, snapshot_store).await;

    // Create discovery and register aggregate
    let discovery = K8sServiceDiscovery::new_static();
    discovery
        .register_aggregate(domain, &addr.ip().to_string(), addr.port())
        .await;

    // Resolve EventQuery - should use registered aggregate
    let mut eq_client = discovery
        .get_event_query(domain)
        .await
        .expect("Should resolve via registered aggregate");

    // Create incomplete EventBook (only event 1, missing 0)
    let incomplete = EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![test_event(1, "ProductEvent1")],
        snapshot: None,
        ..Default::default()
    };

    // Repair via the client we got from discovery
    let repaired = repair_if_needed(&mut eq_client, incomplete)
        .await
        .expect("Repair failed");

    assert_eq!(
        repaired.pages.len(),
        2,
        "Should have all 2 events after repair"
    );
}
