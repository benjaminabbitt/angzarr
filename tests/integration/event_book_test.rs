//! EventBook repository integration tests.

use std::sync::Arc;

use prost_types::Timestamp;
use sqlx::SqlitePool;
use uuid::Uuid;

use evented::proto::{Cover, EventBook, EventPage, Snapshot, Uuid as ProtoUuid};
use evented::repository::EventBookRepository;
use evented::storage::{SqliteEventStore, SqliteSnapshotStore};

async fn test_pool() -> SqlitePool {
    SqlitePool::connect("sqlite::memory:").await.unwrap()
}

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

fn test_snapshot(sequence: u32) -> Snapshot {
    Snapshot {
        sequence,
        state: Some(prost_types::Any {
            type_url: "type.googleapis.com/TestState".to_string(),
            value: vec![10, 20, 30],
        }),
    }
}

fn make_event_book(domain: &str, root: Uuid, events: Vec<EventPage>) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
        }),
        snapshot: None,
        pages: events,
    }
}

async fn setup_shared() -> (
    EventBookRepository,
    Arc<SqliteEventStore>,
    Arc<SqliteSnapshotStore>,
) {
    let pool = test_pool().await;
    let event_store = Arc::new(SqliteEventStore::new(pool.clone()));
    let snapshot_store = Arc::new(SqliteSnapshotStore::new(pool));
    event_store.init().await.unwrap();
    snapshot_store.init().await.unwrap();

    let repo = EventBookRepository::new(event_store.clone(), snapshot_store.clone());
    (repo, event_store, snapshot_store)
}

#[tokio::test]
async fn test_get_empty_aggregate() {
    let (repo, _, _) = setup_shared().await;

    let domain = "test_domain";
    let root = Uuid::new_v4();

    let book = repo.get(domain, root).await.unwrap();

    assert!(book.cover.is_some());
    assert_eq!(book.cover.as_ref().unwrap().domain, domain);
    assert!(book.snapshot.is_none());
    assert!(book.pages.is_empty());
}

#[tokio::test]
async fn test_put_and_get_events() {
    let (repo, _, _) = setup_shared().await;

    let domain = "test_domain";
    let root = Uuid::new_v4();
    let events = vec![test_event(0, "Created"), test_event(1, "Updated")];

    let book = make_event_book(domain, root, events);
    repo.put(&book).await.unwrap();

    let retrieved = repo.get(domain, root).await.unwrap();
    assert_eq!(retrieved.pages.len(), 2);
    assert_eq!(
        retrieved.pages[0].sequence,
        Some(evented::proto::event_page::Sequence::Num(0))
    );
    assert_eq!(
        retrieved.pages[1].sequence,
        Some(evented::proto::event_page::Sequence::Num(1))
    );
}

#[tokio::test]
async fn test_get_with_snapshot_loads_from_snapshot_sequence() {
    let (repo, event_store, snapshot_store) = setup_shared().await;

    let domain = "test_domain";
    let root = Uuid::new_v4();

    // Add events 0-4
    use evented::interfaces::EventStore;
    event_store
        .add(
            domain,
            root,
            vec![
                test_event(0, "Event0"),
                test_event(1, "Event1"),
                test_event(2, "Event2"),
                test_event(3, "Event3"),
                test_event(4, "Event4"),
            ],
        )
        .await
        .unwrap();

    // Store snapshot at sequence 3
    use evented::interfaces::SnapshotStore;
    snapshot_store
        .put(domain, root, test_snapshot(3))
        .await
        .unwrap();

    // Get should load snapshot and events from 3 onwards
    let book = repo.get(domain, root).await.unwrap();

    assert!(book.snapshot.is_some());
    assert_eq!(book.snapshot.as_ref().unwrap().sequence, 3);
    assert_eq!(book.pages.len(), 2); // Events 3 and 4
    assert_eq!(
        book.pages[0].sequence,
        Some(evented::proto::event_page::Sequence::Num(3))
    );
    assert_eq!(
        book.pages[1].sequence,
        Some(evented::proto::event_page::Sequence::Num(4))
    );
}

#[tokio::test]
async fn test_get_from_to_range() {
    let (repo, event_store, _) = setup_shared().await;

    let domain = "test_domain";
    let root = Uuid::new_v4();

    use evented::interfaces::EventStore;
    event_store
        .add(
            domain,
            root,
            vec![
                test_event(0, "Event0"),
                test_event(1, "Event1"),
                test_event(2, "Event2"),
                test_event(3, "Event3"),
                test_event(4, "Event4"),
            ],
        )
        .await
        .unwrap();

    let book = repo.get_from_to(domain, root, 1, 4).await.unwrap();

    assert!(book.snapshot.is_none()); // Range query doesn't include snapshot
    assert_eq!(book.pages.len(), 3); // Events 1, 2, 3
    assert_eq!(
        book.pages[0].sequence,
        Some(evented::proto::event_page::Sequence::Num(1))
    );
    assert_eq!(
        book.pages[2].sequence,
        Some(evented::proto::event_page::Sequence::Num(3))
    );
}

#[tokio::test]
async fn test_put_requires_cover() {
    let (repo, _, _) = setup_shared().await;

    let book = EventBook {
        cover: None,
        snapshot: None,
        pages: vec![test_event(0, "Event")],
    };

    let result = repo.put(&book).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_put_requires_root_in_cover() {
    let (repo, _, _) = setup_shared().await;

    let book = EventBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: None,
        }),
        snapshot: None,
        pages: vec![test_event(0, "Event")],
    };

    let result = repo.put(&book).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multiple_puts_append_events() {
    let (repo, _, _) = setup_shared().await;

    let domain = "test_domain";
    let root = Uuid::new_v4();

    // First put
    let book1 = make_event_book(domain, root, vec![test_event(0, "Created")]);
    repo.put(&book1).await.unwrap();

    // Second put
    let book2 = make_event_book(domain, root, vec![test_event(1, "Updated")]);
    repo.put(&book2).await.unwrap();

    let retrieved = repo.get(domain, root).await.unwrap();
    assert_eq!(retrieved.pages.len(), 2);
}
