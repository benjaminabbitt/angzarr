use super::*;
use crate::proto::{EventPage, Snapshot, SnapshotRetention};
use crate::test_utils::{make_cover, make_event_page};

// ============================================================================
// is_complete Tests
// ============================================================================

#[test]
fn test_is_complete_empty_book() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![],
        snapshot: None,
        ..Default::default()
    };

    assert!(is_complete(&book));
}

#[test]
fn test_is_complete_with_snapshot() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(5), make_event_page(6)],
        snapshot: Some(Snapshot {
            sequence: 5,
            state: None,
            retention: SnapshotRetention::RetentionDefault as i32,
        }),
        ..Default::default()
    };

    assert!(is_complete(&book));
}

#[test]
fn test_is_complete_starts_at_zero() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(0), make_event_page(1), make_event_page(2)],
        snapshot: None,
        ..Default::default()
    };

    assert!(is_complete(&book));
}

#[test]
fn test_is_incomplete_missing_history() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(5), make_event_page(6)], // Missing 0-4
        snapshot: None,
        ..Default::default()
    };

    assert!(!is_complete(&book));
}

#[test]
fn test_is_incomplete_starts_at_nonzero() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(3)], // Missing 0-2
        snapshot: None,
        ..Default::default()
    };

    assert!(!is_complete(&book));
}

#[test]
fn test_is_complete_single_event_at_zero() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(0)],
        snapshot: None,
        ..Default::default()
    };

    assert!(is_complete(&book));
}

#[test]
fn test_is_complete_single_event_not_at_zero() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(1)],
        snapshot: None,
        ..Default::default()
    };

    assert!(!is_complete(&book));
}

#[test]
fn test_is_complete_snapshot_at_zero() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![],
        snapshot: Some(Snapshot {
            sequence: 0,
            state: None,
            retention: SnapshotRetention::RetentionDefault as i32,
        }),
        ..Default::default()
    };

    assert!(is_complete(&book));
}

#[test]
fn test_is_complete_snapshot_without_pages() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![],
        snapshot: Some(Snapshot {
            sequence: 10,
            state: None,
            retention: SnapshotRetention::RetentionDefault as i32,
        }),
        ..Default::default()
    };

    // With a snapshot and no pages, it's considered complete
    assert!(is_complete(&book));
}

#[test]
fn test_is_complete_default_event_book() {
    let book = EventBook::default();
    // Default EventBook has no cover, no pages, no snapshot - considered complete (empty aggregate)
    assert!(is_complete(&book));
}

#[test]
fn test_is_complete_many_events_starting_at_zero() {
    let pages: Vec<EventPage> = (0..100).map(make_event_page).collect();

    let book = EventBook {
        cover: Some(make_cover("test")),
        pages,
        snapshot: None,
        ..Default::default()
    };

    assert!(is_complete(&book));
}

#[test]
fn test_is_complete_gap_in_middle() {
    // This tests a book that starts at 0 but has gaps
    // The is_complete function only checks the first event's sequence
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(0), make_event_page(5), make_event_page(10)],
        snapshot: None,
        ..Default::default()
    };

    // Book starts at 0, so it's considered complete
    // (Gap detection is a separate concern)
    assert!(is_complete(&book));
}

// ============================================================================
// extract_identity Tests
// ============================================================================

#[test]
fn test_extract_identity_success() {
    let root = Uuid::new_v4();
    let book = EventBook {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
        ..Default::default()
    };

    let (domain, extracted_root) = extract_identity(&book).unwrap();
    assert_eq!(domain, "orders");
    assert_eq!(extracted_root, root);
}

#[test]
fn test_extract_identity_missing_cover() {
    let book = EventBook {
        cover: None,
        pages: vec![],
        snapshot: None,
        ..Default::default()
    };

    let result = extract_identity(&book);
    assert!(matches!(result, Err(RepairError::MissingCover)));
}

#[test]
fn test_extract_identity_missing_root() {
    let book = EventBook {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: None,
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
        ..Default::default()
    };

    let result = extract_identity(&book);
    assert!(matches!(result, Err(RepairError::MissingRoot)));
}

#[test]
fn test_extract_identity_invalid_uuid() {
    let book = EventBook {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: vec![1, 2, 3], // Invalid - not 16 bytes
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
        ..Default::default()
    };

    let result = extract_identity(&book);
    assert!(matches!(result, Err(RepairError::InvalidUuid(_))));
}

#[test]
fn test_extract_identity_empty_uuid_bytes() {
    let book = EventBook {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid { value: vec![] }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
        ..Default::default()
    };

    let result = extract_identity(&book);
    assert!(matches!(result, Err(RepairError::InvalidUuid(_))));
}

#[test]
fn test_extract_identity_preserves_domain() {
    let root = Uuid::new_v4();
    let domains = ["orders", "inventory", "fulfillment", "player", "_internal"];

    for domain in domains {
        let book = EventBook {
            cover: Some(crate::proto::Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: None,
            }),
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };

        let (extracted_domain, _) = extract_identity(&book).unwrap();
        assert_eq!(extracted_domain, domain);
    }
}

// ============================================================================
// RepairError Tests
// ============================================================================

#[test]
fn test_repair_error_missing_cover_display() {
    let err = RepairError::MissingCover;
    assert!(err.to_string().contains("missing cover"));
}

#[test]
fn test_repair_error_missing_root_display() {
    let err = RepairError::MissingRoot;
    assert!(err.to_string().contains("missing root"));
}

#[test]
fn test_repair_error_invalid_uuid_display() {
    // Create a uuid error by trying to parse invalid bytes
    let bad_bytes: [u8; 5] = [1, 2, 3, 4, 5];
    let uuid_err = Uuid::from_slice(&bad_bytes).unwrap_err();
    let err = RepairError::InvalidUuid(uuid_err);
    assert!(err.to_string().contains("Invalid UUID"));
}

#[test]
fn test_repair_error_grpc_display() {
    let status = tonic::Status::internal("test error");
    let err: RepairError = status.into();
    assert!(err.to_string().contains("gRPC error"));
}

#[test]
fn test_repair_error_invalid_uri_display() {
    let err = RepairError::InvalidUri("bad://uri".to_string());
    assert!(err.to_string().contains("Invalid URI"));
    assert!(err.to_string().contains("bad://uri"));
}

#[test]
fn test_repair_error_no_event_book_display() {
    let err = RepairError::NoEventBookReturned;
    assert!(err.to_string().contains("No EventBook"));
}

#[test]
fn test_repair_error_from_tonic_status() {
    let status = tonic::Status::not_found("aggregate not found");
    let err: RepairError = status.into();

    match err {
        RepairError::Grpc(boxed_status) => {
            assert_eq!(boxed_status.code(), tonic::Code::NotFound);
            assert!(boxed_status.message().contains("not found"));
        }
        _ => panic!("Expected Grpc error"),
    }
}

// ============================================================================
// EventBookRepairer Tests (construction only - gRPC tests are in grpc_integration)
// ============================================================================

#[test]
fn test_is_complete_function_directly() {
    // For unit tests, we verify the free function directly
    // The actual gRPC tests are in grpc_integration module
    let book_complete = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(0), make_event_page(1)],
        snapshot: None,
        ..Default::default()
    };

    let book_incomplete = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(5)],
        snapshot: None,
        ..Default::default()
    };

    // Test the free function directly since we can't construct repairer without channel
    assert!(is_complete(&book_complete));
    assert!(!is_complete(&book_incomplete));
}

mod grpc_integration {
    use super::*;
    use crate::orchestration::aggregate::DEFAULT_EDITION;
    use crate::proto::event_page;
    use crate::proto::event_query_service_server::EventQueryServiceServer;
    use crate::proto::Snapshot;
    use crate::services::EventQueryService;
    use crate::storage::mock::{MockEventStore, MockSnapshotStore};
    use crate::storage::EventStore;
    use crate::test_utils::make_event_book_with_root;
    use prost_types::Timestamp;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tonic::transport::Server;

    fn test_event(sequence: u32, event_type: &str) -> EventPage {
        EventPage {
            sequence,
            created_at: Some(Timestamp {
                seconds: 1704067200 + sequence as i64,
                nanos: 0,
            }),
            payload: Some(event_page::Payload::Event(prost_types::Any {
                type_url: format!("type.googleapis.com/{}", event_type),
                value: vec![1, 2, 3, sequence as u8],
            })),
        }
    }

    async fn start_event_query_server(
        event_store: Arc<MockEventStore>,
        snapshot_store: Arc<MockSnapshotStore>,
    ) -> SocketAddr {
        start_event_query_server_with_options(event_store, snapshot_store, false).await
    }

    async fn start_event_query_server_with_options(
        event_store: Arc<MockEventStore>,
        snapshot_store: Arc<MockSnapshotStore>,
        enable_snapshots: bool,
    ) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let service =
            EventQueryService::with_options(event_store, snapshot_store, enable_snapshots);

        tokio::spawn(async move {
            Server::builder()
                .add_service(EventQueryServiceServer::new(service))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        addr
    }

    #[tokio::test]
    async fn test_repairer_fetches_complete_event_book() {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let domain = "orders";
        let root = Uuid::new_v4();
        event_store
            .add(
                domain,
                DEFAULT_EDITION,
                root,
                vec![
                    test_event(0, "Created"),
                    test_event(1, "Updated"),
                    test_event(2, "ItemAdded"),
                    test_event(3, "ItemAdded"),
                    test_event(4, "Completed"),
                ],
                "",
            )
            .await
            .unwrap();

        let addr = start_event_query_server(event_store, snapshot_store).await;

        let mut repairer = EventBookRepairer::connect(&addr.to_string()).await.unwrap();

        let incomplete_book =
            make_event_book_with_root(domain, root, vec![test_event(4, "Completed")]);
        assert!(!is_complete(&incomplete_book));

        let repaired = repairer.repair(incomplete_book).await.unwrap();

        assert!(is_complete(&repaired));
        assert_eq!(repaired.pages.len(), 5);
        assert_eq!(repaired.pages[0].sequence, 0);
        assert_eq!(repaired.pages[4].sequence, 4);
    }

    #[tokio::test]
    async fn test_repairer_passes_through_complete_book() {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let addr = start_event_query_server(event_store, snapshot_store).await;

        let mut repairer = EventBookRepairer::connect(&addr.to_string()).await.unwrap();

        let complete_book = make_event_book_with_root(
            "orders",
            Uuid::new_v4(),
            vec![test_event(0, "Created"), test_event(1, "Updated")],
        );
        assert!(is_complete(&complete_book));

        let result = repairer.repair(complete_book.clone()).await.unwrap();

        assert_eq!(result.pages.len(), 2);
        assert_eq!(result.pages[0].sequence, 0);
    }

    #[tokio::test]
    async fn test_repairer_with_snapshot_in_storage() {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let domain = "orders";
        let root = Uuid::new_v4();
        let events: Vec<EventPage> = (0..10)
            .map(|i| test_event(i, &format!("Event{}", i)))
            .collect();
        event_store
            .add(domain, DEFAULT_EDITION, root, events, "")
            .await
            .unwrap();

        use crate::storage::SnapshotStore;
        snapshot_store
            .put(
                domain,
                DEFAULT_EDITION,
                root,
                Snapshot {
                    sequence: 5,
                    state: Some(prost_types::Any {
                        type_url: "TestState".to_string(),
                        value: vec![1, 2, 3],
                    }),
                    retention: SnapshotRetention::RetentionDefault as i32,
                },
            )
            .await
            .unwrap();

        let addr = start_event_query_server_with_options(event_store, snapshot_store, true).await;

        let mut repairer = EventBookRepairer::connect(&addr.to_string()).await.unwrap();

        let incomplete_book = make_event_book_with_root(
            domain,
            root,
            vec![test_event(8, "Event8"), test_event(9, "Event9")],
        );
        assert!(!is_complete(&incomplete_book));

        let repaired = repairer.repair(incomplete_book).await.unwrap();

        assert!(is_complete(&repaired));
        assert!(repaired.snapshot.is_some());
        assert_eq!(repaired.snapshot.as_ref().unwrap().sequence, 5);
        assert_eq!(repaired.pages.len(), 4); // Events 6,7,8,9 (after snapshot at 5)
        assert_eq!(repaired.pages[0].sequence, 6);
    }

    #[tokio::test]
    async fn test_repairer_empty_aggregate_returns_empty() {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let addr = start_event_query_server(event_store, snapshot_store).await;

        let mut repairer = EventBookRepairer::connect(&addr.to_string()).await.unwrap();

        let root = Uuid::new_v4();
        let incomplete_book =
            make_event_book_with_root("orders", root, vec![test_event(5, "Event5")]);

        let repaired = repairer.repair(incomplete_book).await.unwrap();

        assert!(is_complete(&repaired));
        assert!(repaired.pages.is_empty());
    }
}
