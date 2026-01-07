//! Storage integration tests.

use prost_types::Timestamp;
use sqlx::SqlitePool;
use uuid::Uuid;

use evented::interfaces::{EventStore, SnapshotStore};
use evented::proto::{EventPage, Snapshot};
use evented::storage::{SqliteEventStore, SqliteSnapshotStore};

/// Create an in-memory SQLite pool for testing.
async fn test_pool() -> SqlitePool {
    SqlitePool::connect("sqlite::memory:").await.unwrap()
}

/// Create a test event page.
fn test_event(sequence: u32, event_type: &str) -> EventPage {
    EventPage {
        sequence: Some(evented::proto::event_page::Sequence::Num(sequence)),
        created_at: Some(Timestamp {
            seconds: 1704067200 + sequence as i64,
            nanos: 0,
        }),
        event: Some(prost_types::Any {
            type_url: format!("type.googleapis.com/{}", event_type),
            value: vec![1, 2, 3, sequence as u8],
        }),
        synchronous: false,
    }
}

/// Create a test snapshot.
fn test_snapshot(sequence: u32) -> Snapshot {
    Snapshot {
        sequence,
        state: Some(prost_types::Any {
            type_url: "type.googleapis.com/TestState".to_string(),
            value: vec![10, 20, 30],
        }),
    }
}

mod event_store {
    use super::*;

    #[tokio::test]
    async fn test_add_get_events() {
        let pool = test_pool().await;
        let store = SqliteEventStore::new(pool);
        store.init().await.unwrap();

        let domain = "test_domain";
        let root = Uuid::new_v4();
        let events = vec![test_event(0, "TestCreated"), test_event(1, "TestUpdated")];

        store.add(domain, root, events).await.unwrap();

        let retrieved = store.get(domain, root).await.unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(
            retrieved[0].sequence,
            Some(evented::proto::event_page::Sequence::Num(0))
        );
        assert_eq!(
            retrieved[1].sequence,
            Some(evented::proto::event_page::Sequence::Num(1))
        );
    }

    #[tokio::test]
    async fn test_get_from_sequence() {
        let pool = test_pool().await;
        let store = SqliteEventStore::new(pool);
        store.init().await.unwrap();

        let domain = "test_domain";
        let root = Uuid::new_v4();
        let events = vec![
            test_event(0, "Event0"),
            test_event(1, "Event1"),
            test_event(2, "Event2"),
            test_event(3, "Event3"),
        ];

        store.add(domain, root, events).await.unwrap();

        let from_2 = store.get_from(domain, root, 2).await.unwrap();
        assert_eq!(from_2.len(), 2);
        assert_eq!(
            from_2[0].sequence,
            Some(evented::proto::event_page::Sequence::Num(2))
        );
        assert_eq!(
            from_2[1].sequence,
            Some(evented::proto::event_page::Sequence::Num(3))
        );
    }

    #[tokio::test]
    async fn test_get_from_to_range() {
        let pool = test_pool().await;
        let store = SqliteEventStore::new(pool);
        store.init().await.unwrap();

        let domain = "test_domain";
        let root = Uuid::new_v4();
        let events = vec![
            test_event(0, "Event0"),
            test_event(1, "Event1"),
            test_event(2, "Event2"),
            test_event(3, "Event3"),
            test_event(4, "Event4"),
        ];

        store.add(domain, root, events).await.unwrap();

        let range = store.get_from_to(domain, root, 1, 4).await.unwrap();
        assert_eq!(range.len(), 3);
        assert_eq!(
            range[0].sequence,
            Some(evented::proto::event_page::Sequence::Num(1))
        );
        assert_eq!(
            range[2].sequence,
            Some(evented::proto::event_page::Sequence::Num(3))
        );
    }

    #[tokio::test]
    async fn test_list_roots() {
        let pool = test_pool().await;
        let store = SqliteEventStore::new(pool);
        store.init().await.unwrap();

        let domain = "test_domain";
        let root1 = Uuid::new_v4();
        let root2 = Uuid::new_v4();
        let root3 = Uuid::new_v4();

        store
            .add(domain, root1, vec![test_event(0, "Event")])
            .await
            .unwrap();
        store
            .add(domain, root2, vec![test_event(0, "Event")])
            .await
            .unwrap();
        store
            .add("other_domain", root3, vec![test_event(0, "Event")])
            .await
            .unwrap();

        let roots = store.list_roots(domain).await.unwrap();
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&root1));
        assert!(roots.contains(&root2));
        assert!(!roots.contains(&root3));
    }

    #[tokio::test]
    async fn test_get_next_sequence() {
        let pool = test_pool().await;
        let store = SqliteEventStore::new(pool);
        store.init().await.unwrap();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        // No events yet
        let seq = store.get_next_sequence(domain, root).await.unwrap();
        assert_eq!(seq, 0);

        // Add some events
        store
            .add(
                domain,
                root,
                vec![test_event(0, "Event0"), test_event(1, "Event1")],
            )
            .await
            .unwrap();

        let seq = store.get_next_sequence(domain, root).await.unwrap();
        assert_eq!(seq, 2);
    }

    #[tokio::test]
    async fn test_get_empty_returns_empty_vec() {
        let pool = test_pool().await;
        let store = SqliteEventStore::new(pool);
        store.init().await.unwrap();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        let events = store.get(domain, root).await.unwrap();
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_events_isolated_by_domain() {
        let pool = test_pool().await;
        let store = SqliteEventStore::new(pool);
        store.init().await.unwrap();

        let root = Uuid::new_v4();

        store
            .add("domain_a", root, vec![test_event(0, "EventA")])
            .await
            .unwrap();
        store
            .add("domain_b", root, vec![test_event(0, "EventB")])
            .await
            .unwrap();

        let events_a = store.get("domain_a", root).await.unwrap();
        let events_b = store.get("domain_b", root).await.unwrap();

        assert_eq!(events_a.len(), 1);
        assert_eq!(events_b.len(), 1);
        assert_ne!(events_a[0].event, events_b[0].event);
    }
}

mod snapshot_store {
    use super::*;

    #[tokio::test]
    async fn test_put_get_snapshot() {
        let pool = test_pool().await;
        let store = SqliteSnapshotStore::new(pool);
        store.init().await.unwrap();

        let domain = "test_domain";
        let root = Uuid::new_v4();
        let snapshot = test_snapshot(5);

        store.put(domain, root, snapshot.clone()).await.unwrap();

        let retrieved = store.get(domain, root).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.sequence, 5);
    }

    #[tokio::test]
    async fn test_get_nonexistent_returns_none() {
        let pool = test_pool().await;
        let store = SqliteSnapshotStore::new(pool);
        store.init().await.unwrap();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        let retrieved = store.get(domain, root).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_put_overwrites_existing() {
        let pool = test_pool().await;
        let store = SqliteSnapshotStore::new(pool);
        store.init().await.unwrap();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        store.put(domain, root, test_snapshot(5)).await.unwrap();
        store.put(domain, root, test_snapshot(10)).await.unwrap();

        let retrieved = store.get(domain, root).await.unwrap().unwrap();
        assert_eq!(retrieved.sequence, 10);
    }

    #[tokio::test]
    async fn test_delete_snapshot() {
        let pool = test_pool().await;
        let store = SqliteSnapshotStore::new(pool);
        store.init().await.unwrap();

        let domain = "test_domain";
        let root = Uuid::new_v4();

        store.put(domain, root, test_snapshot(5)).await.unwrap();
        assert!(store.get(domain, root).await.unwrap().is_some());

        store.delete(domain, root).await.unwrap();
        assert!(store.get(domain, root).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_snapshots_isolated_by_domain() {
        let pool = test_pool().await;
        let store = SqliteSnapshotStore::new(pool);
        store.init().await.unwrap();

        let root = Uuid::new_v4();

        store.put("domain_a", root, test_snapshot(5)).await.unwrap();
        store
            .put("domain_b", root, test_snapshot(10))
            .await
            .unwrap();

        let snap_a = store.get("domain_a", root).await.unwrap().unwrap();
        let snap_b = store.get("domain_b", root).await.unwrap().unwrap();

        assert_eq!(snap_a.sequence, 5);
        assert_eq!(snap_b.sequence, 10);
    }
}
