use uuid::Uuid;

use crate::proto::{EventPage, Snapshot};
use crate::storage::{EventStore, SnapshotStore};

use super::*;

#[tokio::test]
async fn test_mock_event_store_add_and_get() {
    let store = MockEventStore::new();
    let root = Uuid::new_v4();

    let events = vec![EventPage {
        sequence: 0,
        payload: Some(crate::proto::event_page::Payload::Event(prost_types::Any {
            type_url: "test.Event".to_string(),
            value: vec![],
        })),
        created_at: None,
    }];

    store
        .add("orders", "test", root, events, "corr-123")
        .await
        .unwrap();

    let retrieved = store.get("orders", "test", root).await.unwrap();
    assert_eq!(retrieved.len(), 1);
}

#[tokio::test]
async fn test_mock_event_store_get_by_correlation() {
    let store = MockEventStore::new();
    let root1 = Uuid::new_v4();
    let root2 = Uuid::new_v4();

    let event1 = EventPage {
        sequence: 0,
        payload: Some(crate::proto::event_page::Payload::Event(prost_types::Any {
            type_url: "orders.Created".to_string(),
            value: vec![],
        })),
        created_at: None,
    };

    let event2 = EventPage {
        sequence: 0,
        payload: Some(crate::proto::event_page::Payload::Event(prost_types::Any {
            type_url: "payment.Confirmed".to_string(),
            value: vec![],
        })),
        created_at: None,
    };

    // Add events with same correlation_id across different domains
    store
        .add("orders", "test", root1, vec![event1], "tx-abc")
        .await
        .unwrap();
    store
        .add("payment", "test", root2, vec![event2], "tx-abc")
        .await
        .unwrap();

    // Query by correlation_id
    let books = store.get_by_correlation("tx-abc").await.unwrap();
    assert_eq!(books.len(), 2);

    // Query with different correlation_id returns empty
    let empty = store.get_by_correlation("tx-xyz").await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_get_until_timestamp_filters_by_created_at() {
    let store = MockEventStore::new();
    let root = Uuid::new_v4();

    let events = vec![
        EventPage {
            sequence: 0,
            payload: Some(crate::proto::event_page::Payload::Event(prost_types::Any {
                type_url: "test.Event0".to_string(),
                value: vec![],
            })),
            created_at: Some(prost_types::Timestamp {
                seconds: 1704067200, // 2024-01-01T00:00:00Z
                nanos: 0,
            }),
        },
        EventPage {
            sequence: 1,
            payload: Some(crate::proto::event_page::Payload::Event(prost_types::Any {
                type_url: "test.Event1".to_string(),
                value: vec![],
            })),
            created_at: Some(prost_types::Timestamp {
                seconds: 1704153600, // 2024-01-02T00:00:00Z
                nanos: 0,
            }),
        },
        EventPage {
            sequence: 2,
            payload: Some(crate::proto::event_page::Payload::Event(prost_types::Any {
                type_url: "test.Event2".to_string(),
                value: vec![],
            })),
            created_at: Some(prost_types::Timestamp {
                seconds: 1704240000, // 2024-01-03T00:00:00Z
                nanos: 0,
            }),
        },
    ];
    store.add("orders", "test", root, events, "").await.unwrap();

    // Query as-of Jan 2 — should return events 0 and 1
    let result = store
        .get_until_timestamp("orders", "test", root, "2024-01-02T00:00:00Z")
        .await
        .unwrap();
    assert_eq!(result.len(), 2);

    // Query as-of Jan 1 — should return event 0 only
    let result = store
        .get_until_timestamp("orders", "test", root, "2024-01-01T00:00:00Z")
        .await
        .unwrap();
    assert_eq!(result.len(), 1);

    // Query before any events — should return empty
    let result = store
        .get_until_timestamp("orders", "test", root, "2023-12-31T00:00:00Z")
        .await
        .unwrap();
    assert!(result.is_empty());

    // Query after all events — should return all
    let result = store
        .get_until_timestamp("orders", "test", root, "2024-01-04T00:00:00Z")
        .await
        .unwrap();
    assert_eq!(result.len(), 3);
}

#[tokio::test]
async fn test_get_until_timestamp_excludes_events_without_timestamp() {
    let store = MockEventStore::new();
    let root = Uuid::new_v4();

    let events = vec![EventPage {
        sequence: 0,
        payload: Some(crate::proto::event_page::Payload::Event(prost_types::Any {
            type_url: "test.Event".to_string(),
            value: vec![],
        })),
        created_at: None,
    }];
    store.add("orders", "test", root, events, "").await.unwrap();

    let result = store
        .get_until_timestamp("orders", "test", root, "2024-01-02T00:00:00Z")
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_get_until_timestamp_invalid_format() {
    let store = MockEventStore::new();
    let root = Uuid::new_v4();

    let result = store
        .get_until_timestamp("orders", "test", root, "not-a-timestamp")
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mock_snapshot_store() {
    let store = MockSnapshotStore::new();
    let root = Uuid::new_v4();

    let snapshot = Snapshot {
        sequence: 5,
        state: None,
        retention: crate::proto::SnapshotRetention::RetentionDefault as i32,
    };

    store
        .put("orders", "test", root, snapshot.clone())
        .await
        .unwrap();

    let retrieved = store.get("orders", "test", root).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().sequence, 5);
}
