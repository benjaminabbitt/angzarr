use super::*;
use crate::proto::{event_page, EventPage, Snapshot};
use crate::storage::mock::{MockEventStore, MockSnapshotStore};
use crate::test_utils::{make_event_book_with_root, make_event_page};

#[tokio::test]
async fn test_get_returns_empty_book_for_new_aggregate() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let repo = EventBookRepository::new(event_store, snapshot_store);

    let root = Uuid::new_v4();
    let book = repo.get("orders", "test", root).await.unwrap();

    assert!(book.pages.is_empty());
    assert!(book.snapshot.is_none());
    assert_eq!(book.cover.as_ref().unwrap().domain, "orders");
}

#[tokio::test]
async fn test_put_and_get_roundtrip() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let repo = EventBookRepository::new(event_store, snapshot_store);

    let root = Uuid::new_v4();
    let book = make_event_book_with_root(
        "orders",
        root,
        vec![make_event_page(0), make_event_page(1)],
    );

    repo.put("test", &book).await.unwrap();

    let retrieved = repo.get("orders", "test", root).await.unwrap();
    assert_eq!(retrieved.pages.len(), 2);
}

#[tokio::test]
async fn test_get_with_snapshot_starts_from_snapshot_sequence() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let repo = EventBookRepository::new(event_store.clone(), snapshot_store.clone());

    let root = Uuid::new_v4();

    // Add events 0-4
    event_store
        .add("orders", "test", root, (0..5).map(make_event_page).collect(), "")
        .await
        .unwrap();

    // Add snapshot at sequence 3
    snapshot_store
        .put(
            "orders",
            "test",
            root,
            Snapshot {
                sequence: 3,
                state: None,
            },
        )
        .await
        .unwrap();

    let book = repo.get("orders", "test", root).await.unwrap();

    // Should only have events AFTER snapshot (snapshot contains seq 3, so load from 4)
    assert_eq!(book.pages.len(), 1); // Only event 4
    assert!(book.snapshot.is_some());
    assert_eq!(book.snapshot.as_ref().unwrap().sequence, 3);
}

#[tokio::test]
async fn test_get_from_to_returns_range() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let repo = EventBookRepository::new(event_store.clone(), snapshot_store);

    let root = Uuid::new_v4();

    event_store
        .add(
            "orders",
            "test",
            root,
            (0..10).map(make_event_page).collect(),
            "",
        )
        .await
        .unwrap();

    let book = repo.get_from_to("orders", "test", root, 3, 7).await.unwrap();

    assert_eq!(book.pages.len(), 4); // Events 3, 4, 5, 6
    assert!(book.snapshot.is_none()); // Range query doesn't include snapshot
}

#[tokio::test]
async fn test_put_missing_cover_returns_error() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let repo = EventBookRepository::new(event_store, snapshot_store);

    let book = EventBook {
        cover: None,
        pages: vec![],
        snapshot: None,
        snapshot_state: None,
    };

    let result = repo.put("test", &book).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_put_missing_root_returns_error() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let repo = EventBookRepository::new(event_store, snapshot_store);

    let book = EventBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: None,
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
        snapshot_state: None,
    };

    let result = repo.put("test", &book).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_put_invalid_uuid_returns_error() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let repo = EventBookRepository::new(event_store, snapshot_store);

    let book = EventBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: vec![1, 2, 3], // Invalid: not 16 bytes
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
        snapshot_state: None,
    };

    let result = repo.put("test", &book).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_propagates_store_error() {
    let event_store = Arc::new(MockEventStore::new());
    event_store.set_fail_on_get(true).await;
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let repo = EventBookRepository::new(event_store, snapshot_store);

    let result = repo.get("orders", "test", Uuid::new_v4()).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_put_propagates_store_error() {
    let event_store = Arc::new(MockEventStore::new());
    event_store.set_fail_on_add(true).await;
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let repo = EventBookRepository::new(event_store, snapshot_store);

    let root = Uuid::new_v4();
    let book = make_event_book_with_root("orders", root, vec![]);

    let result = repo.put("test", &book).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_with_config_snapshot_read_disabled_ignores_snapshot() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let repo =
        EventBookRepository::with_config(event_store.clone(), snapshot_store.clone(), false);

    let root = Uuid::new_v4();

    // Add events 0-4
    event_store
        .add("orders", "test", root, (0..5).map(make_event_page).collect(), "")
        .await
        .unwrap();

    // Add snapshot at sequence 3
    snapshot_store
        .put(
            "orders",
            "test",
            root,
            Snapshot {
                sequence: 3,
                state: None,
            },
        )
        .await
        .unwrap();

    let book = repo.get("orders", "test", root).await.unwrap();

    // With snapshot reading disabled, should load ALL events from beginning
    assert_eq!(book.pages.len(), 5);
    assert!(book.snapshot.is_none());
}

#[tokio::test]
async fn test_with_config_snapshot_read_enabled_uses_snapshot() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());
    let repo =
        EventBookRepository::with_config(event_store.clone(), snapshot_store.clone(), true);

    let root = Uuid::new_v4();

    // Add events 0-4
    event_store
        .add("orders", "test", root, (0..5).map(make_event_page).collect(), "")
        .await
        .unwrap();

    // Add snapshot at sequence 3
    snapshot_store
        .put(
            "orders",
            "test",
            root,
            Snapshot {
                sequence: 3,
                state: None,
            },
        )
        .await
        .unwrap();

    let book = repo.get("orders", "test", root).await.unwrap();

    // With snapshot reading enabled, should load from snapshot sequence + 1
    assert_eq!(book.pages.len(), 1); // Only event 4 (snapshot contains through seq 3)
    assert!(book.snapshot.is_some());
    assert_eq!(book.snapshot.as_ref().unwrap().sequence, 3);
}

#[tokio::test]
async fn test_with_config_defaults_match_new_constructor() {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());

    // with_config(true) should behave the same as new()
    let repo_new = EventBookRepository::new(event_store.clone(), snapshot_store.clone());
    let repo_config =
        EventBookRepository::with_config(event_store.clone(), snapshot_store.clone(), true);

    let root = Uuid::new_v4();

    event_store
        .add("orders", "test", root, (0..3).map(make_event_page).collect(), "")
        .await
        .unwrap();

    snapshot_store
        .put(
            "orders",
            "test",
            root,
            Snapshot {
                sequence: 2,
                state: None,
            },
        )
        .await
        .unwrap();

    let book_new = repo_new.get("orders", "test", root).await.unwrap();
    let book_config = repo_config.get("orders", "test", root).await.unwrap();

    assert_eq!(book_new.pages.len(), book_config.pages.len());
    assert_eq!(book_new.snapshot.is_some(), book_config.snapshot.is_some());
}

mod mock_integration {
    use super::*;
    use crate::storage::mock::{MockEventStore, MockSnapshotStore};
    use crate::test_utils::make_event_book_with_root;
    use prost_types::{Any, Timestamp};

    fn test_event(sequence: u32, event_type: &str) -> EventPage {
        EventPage {
            sequence: Some(event_page::Sequence::Num(sequence)),
            created_at: Some(Timestamp {
                seconds: 1704067200 + sequence as i64,
                nanos: 0,
            }),
            event: Some(Any {
                type_url: format!("type.googleapis.com/{}", event_type),
                value: vec![1, 2, 3, sequence as u8],
            }),
        }
    }

    fn test_snapshot(sequence: u32) -> Snapshot {
        Snapshot {
            sequence,
            state: Some(Any {
                type_url: "type.googleapis.com/TestState".to_string(),
                value: vec![10, 20, 30],
            }),
        }
    }

    fn setup_shared() -> (
        EventBookRepository,
        Arc<MockEventStore>,
        Arc<MockSnapshotStore>,
    ) {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());
        let repo = EventBookRepository::new(event_store.clone(), snapshot_store.clone());
        (repo, event_store, snapshot_store)
    }

    #[tokio::test]
    async fn test_get_empty_aggregate() {
        let (repo, _, _) = setup_shared();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        let book = repo.get(domain, "test", root).await.unwrap();

        assert!(book.cover.is_some());
        assert_eq!(book.cover.as_ref().unwrap().domain, domain);
        assert!(book.snapshot.is_none());
        assert!(book.pages.is_empty());
    }

    #[tokio::test]
    async fn test_put_and_get_events() {
        let (repo, _, _) = setup_shared();

        let domain = "test_domain";
        let root = Uuid::new_v4();
        let events = vec![test_event(0, "Created"), test_event(1, "Updated")];

        let book = make_event_book_with_root(domain, root, events);
        repo.put("test", &book).await.unwrap();

        let retrieved = repo.get(domain, "test", root).await.unwrap();
        assert_eq!(retrieved.pages.len(), 2);
        assert_eq!(
            retrieved.pages[0].sequence,
            Some(event_page::Sequence::Num(0))
        );
        assert_eq!(
            retrieved.pages[1].sequence,
            Some(event_page::Sequence::Num(1))
        );
    }

    #[tokio::test]
    async fn test_get_with_snapshot_loads_from_snapshot_sequence() {
        let (repo, event_store, snapshot_store) = setup_shared();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        use crate::storage::EventStore;
        event_store
            .add(
                domain,
                "test",
                root,
                vec![
                    test_event(0, "Event0"),
                    test_event(1, "Event1"),
                    test_event(2, "Event2"),
                    test_event(3, "Event3"),
                    test_event(4, "Event4"),
                ],
                "",
            )
            .await
            .unwrap();

        use crate::storage::SnapshotStore;
        snapshot_store
            .put(domain, "test", root, test_snapshot(3))
            .await
            .unwrap();

        let book = repo.get(domain, "test", root).await.unwrap();

        assert!(book.snapshot.is_some());
        assert_eq!(book.snapshot.as_ref().unwrap().sequence, 3);
        // Snapshot contains state through seq 3, so only events 4+ are loaded
        assert_eq!(book.pages.len(), 1);
        assert_eq!(book.pages[0].sequence, Some(event_page::Sequence::Num(4)));
    }

    #[tokio::test]
    async fn test_get_from_to_range() {
        let (repo, event_store, _) = setup_shared();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        use crate::storage::EventStore;
        event_store
            .add(
                domain,
                "test",
                root,
                vec![
                    test_event(0, "Event0"),
                    test_event(1, "Event1"),
                    test_event(2, "Event2"),
                    test_event(3, "Event3"),
                    test_event(4, "Event4"),
                ],
                "",
            )
            .await
            .unwrap();

        let book = repo.get_from_to(domain, "test", root, 1, 4).await.unwrap();

        assert!(book.snapshot.is_none());
        assert_eq!(book.pages.len(), 3);
        assert_eq!(book.pages[0].sequence, Some(event_page::Sequence::Num(1)));
        assert_eq!(book.pages[2].sequence, Some(event_page::Sequence::Num(3)));
    }

    #[tokio::test]
    async fn test_multiple_puts_append_events() {
        let (repo, _, _) = setup_shared();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        let book1 = make_event_book_with_root(domain, root, vec![test_event(0, "Created")]);
        repo.put("test", &book1).await.unwrap();

        let book2 = make_event_book_with_root(domain, root, vec![test_event(1, "Updated")]);
        repo.put("test", &book2).await.unwrap();

        let retrieved = repo.get(domain, "test", root).await.unwrap();
        assert_eq!(retrieved.pages.len(), 2);
    }

    #[tokio::test]
    async fn test_get_with_snapshot_read_disabled_ignores_snapshot() {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());
        let repo = EventBookRepository::with_config(
            event_store.clone(),
            snapshot_store.clone(),
            false,
        );

        let domain = "test_domain";
        let root = Uuid::new_v4();

        use crate::storage::EventStore;
        event_store
            .add(
                domain,
                "test",
                root,
                vec![
                    test_event(0, "Event0"),
                    test_event(1, "Event1"),
                    test_event(2, "Event2"),
                    test_event(3, "Event3"),
                    test_event(4, "Event4"),
                ],
                "",
            )
            .await
            .unwrap();

        use crate::storage::SnapshotStore;
        snapshot_store
            .put(domain, "test", root, test_snapshot(3))
            .await
            .unwrap();

        let book = repo.get(domain, "test", root).await.unwrap();

        assert!(book.snapshot.is_none());
        assert_eq!(book.pages.len(), 5);
        assert_eq!(book.pages[0].sequence, Some(event_page::Sequence::Num(0)));
        assert_eq!(book.pages[4].sequence, Some(event_page::Sequence::Num(4)));
    }

    #[tokio::test]
    async fn test_get_temporal_by_time_skips_snapshots() {
        let (repo, event_store, snapshot_store) = setup_shared();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        // Events at 1-second intervals starting at 2024-01-01T00:00:00Z
        use crate::storage::EventStore;
        event_store
            .add(
                domain,
                "test",
                root,
                vec![
                    test_event(0, "Event0"),
                    test_event(1, "Event1"),
                    test_event(2, "Event2"),
                    test_event(3, "Event3"),
                    test_event(4, "Event4"),
                ],
                "",
            )
            .await
            .unwrap();

        // Snapshot at sequence 3 — should be ignored for temporal queries
        use crate::storage::SnapshotStore;
        snapshot_store
            .put(domain, "test", root, test_snapshot(3))
            .await
            .unwrap();

        // Query as-of 2 seconds after epoch (should return events 0, 1, 2)
        let book = repo
            .get_temporal_by_time(domain, "test", root, "2024-01-01T00:00:02+00:00")
            .await
            .unwrap();

        assert!(book.snapshot.is_none()); // No snapshot in temporal query
        assert_eq!(book.pages.len(), 3); // Events 0, 1, 2
    }

    #[tokio::test]
    async fn test_get_temporal_by_sequence_skips_snapshots() {
        let (repo, event_store, snapshot_store) = setup_shared();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        use crate::storage::EventStore;
        event_store
            .add(
                domain,
                "test",
                root,
                vec![
                    test_event(0, "Event0"),
                    test_event(1, "Event1"),
                    test_event(2, "Event2"),
                    test_event(3, "Event3"),
                    test_event(4, "Event4"),
                ],
                "",
            )
            .await
            .unwrap();

        // Snapshot at sequence 3 — should be ignored
        use crate::storage::SnapshotStore;
        snapshot_store
            .put(domain, "test", root, test_snapshot(3))
            .await
            .unwrap();

        // Query as-of sequence 2 — should return events 0, 1, 2
        let book = repo
            .get_temporal_by_sequence(domain, "test", root, 2)
            .await
            .unwrap();

        assert!(book.snapshot.is_none());
        assert_eq!(book.pages.len(), 3);
        assert_eq!(book.pages[0].sequence, Some(event_page::Sequence::Num(0)));
        assert_eq!(book.pages[2].sequence, Some(event_page::Sequence::Num(2)));
    }

    #[tokio::test]
    async fn test_get_temporal_by_sequence_zero() {
        let (repo, event_store, _) = setup_shared();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        use crate::storage::EventStore;
        event_store
            .add(
                domain,
                "test",
                root,
                vec![test_event(0, "Event0"), test_event(1, "Event1")],
                "",
            )
            .await
            .unwrap();

        // Query as-of sequence 0 — should return only event 0
        let book = repo
            .get_temporal_by_sequence(domain, "test", root, 0)
            .await
            .unwrap();

        assert_eq!(book.pages.len(), 1);
        assert_eq!(book.pages[0].sequence, Some(event_page::Sequence::Num(0)));
    }
}
