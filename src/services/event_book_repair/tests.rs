use super::*;
use crate::proto::{EventPage, Snapshot};
use crate::test_utils::{make_cover, make_event_page};

#[test]
fn test_is_complete_empty_book() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![],
        snapshot: None,
        snapshot_state: None,
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
        }),
        snapshot_state: None,
    };

    assert!(is_complete(&book));
}

#[test]
fn test_is_complete_starts_at_zero() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(0), make_event_page(1), make_event_page(2)],
        snapshot: None,
        snapshot_state: None,
    };

    assert!(is_complete(&book));
}

#[test]
fn test_is_incomplete_missing_history() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(5), make_event_page(6)], // Missing 0-4
        snapshot: None,
        snapshot_state: None,
    };

    assert!(!is_complete(&book));
}

#[test]
fn test_is_incomplete_starts_at_nonzero() {
    let book = EventBook {
        cover: Some(make_cover("test")),
        pages: vec![make_event_page(3)], // Missing 0-2
        snapshot: None,
        snapshot_state: None,
    };

    assert!(!is_complete(&book));
}

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
        snapshot_state: None,
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
        snapshot_state: None,
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
        snapshot_state: None,
    };

    let result = extract_identity(&book);
    assert!(matches!(result, Err(RepairError::MissingRoot)));
}

mod grpc_integration {
    use super::*;
    use crate::orchestration::aggregate::DEFAULT_EDITION;
    use crate::proto::event_query_server::EventQueryServer;
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
            sequence: Some(Sequence::Num(sequence)),
            created_at: Some(Timestamp {
                seconds: 1704067200 + sequence as i64,
                nanos: 0,
            }),
            event: Some(prost_types::Any {
                type_url: format!("type.googleapis.com/{}", event_type),
                value: vec![1, 2, 3, sequence as u8],
            }),
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
                .add_service(EventQueryServer::new(service))
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

        let incomplete_book = make_event_book_with_root(domain, root, vec![test_event(4, "Completed")]);
        assert!(!is_complete(&incomplete_book));

        let repaired = repairer.repair(incomplete_book).await.unwrap();

        assert!(is_complete(&repaired));
        assert_eq!(repaired.pages.len(), 5);
        assert_eq!(repaired.pages[0].sequence, Some(Sequence::Num(0)));
        assert_eq!(repaired.pages[4].sequence, Some(Sequence::Num(4)));
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
        assert_eq!(result.pages[0].sequence, Some(Sequence::Num(0)));
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
        event_store.add(domain, DEFAULT_EDITION, root, events, "").await.unwrap();

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
                },
            )
            .await
            .unwrap();

        let addr =
            start_event_query_server_with_options(event_store, snapshot_store, true).await;

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
        assert_eq!(repaired.pages[0].sequence, Some(Sequence::Num(6)));
    }

    #[tokio::test]
    async fn test_repairer_empty_aggregate_returns_empty() {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let addr = start_event_query_server(event_store, snapshot_store).await;

        let mut repairer = EventBookRepairer::connect(&addr.to_string()).await.unwrap();

        let root = Uuid::new_v4();
        let incomplete_book = make_event_book_with_root("orders", root, vec![test_event(5, "Event5")]);

        let repaired = repairer.repair(incomplete_book).await.unwrap();

        assert!(is_complete(&repaired));
        assert!(repaired.pages.is_empty());
    }
}
