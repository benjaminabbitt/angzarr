use super::*;
use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::proto::{event_page, EventPage, SequenceRange, TemporalQuery};
use crate::storage::mock::{MockEventStore, MockSnapshotStore};
use prost_types::{Any, Timestamp};
use tokio_stream::StreamExt;

fn create_test_service_with_mocks(
    event_store: Arc<MockEventStore>,
    snapshot_store: Arc<MockSnapshotStore>,
) -> EventQueryService {
    EventQueryService::new(event_store, snapshot_store)
}

fn create_default_test_service() -> (
    EventQueryService,
    Arc<MockEventStore>,
    Arc<MockSnapshotStore>,
) {
    let event_store = Arc::new(MockEventStore::new());
    let snapshot_store = Arc::new(MockSnapshotStore::new());

    let service = create_test_service_with_mocks(event_store.clone(), snapshot_store.clone());

    (service, event_store, snapshot_store)
}

#[tokio::test]
async fn test_get_event_book_empty_aggregate() {
    let (service, _, _) = create_default_test_service();
    let root = uuid::Uuid::new_v4();

    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_event_book(Request::new(query)).await;

    assert!(response.is_ok());
    let book = response.unwrap().into_inner();
    assert!(book.pages.is_empty());
}

#[tokio::test]
async fn test_get_event_book_with_data() {
    let (service, event_store, _) = create_default_test_service();
    let root = uuid::Uuid::new_v4();

    let events = vec![EventPage {
        sequence: Some(event_page::Sequence::Num(0)),
        event: Some(Any {
            type_url: "test.Event".to_string(),
            value: vec![],
        }),
        created_at: None,
    }];
    event_store.add("orders", DEFAULT_EDITION, root, events, "").await.unwrap();

    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_event_book(Request::new(query)).await;

    assert!(response.is_ok());
    let book = response.unwrap().into_inner();
    assert_eq!(book.pages.len(), 1);
}

#[tokio::test]
async fn test_get_event_book_missing_root() {
    let (service, _, _) = create_default_test_service();

    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: None,
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_event_book(Request::new(query)).await;

    assert!(response.is_err());
    let status = response.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_get_event_book_invalid_uuid() {
    let (service, _, _) = create_default_test_service();

    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: vec![1, 2, 3], // Invalid UUID
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_event_book(Request::new(query)).await;

    assert!(response.is_err());
    let status = response.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_get_event_book_with_range() {
    let (service, event_store, _) = create_default_test_service();
    let root = uuid::Uuid::new_v4();

    // Add multiple events
    for i in 0..5 {
        let events = vec![EventPage {
            sequence: Some(event_page::Sequence::Num(i)),
            event: Some(Any {
                type_url: format!("test.Event{}", i),
                value: vec![],
            }),
            created_at: None,
        }];
        event_store.add("orders", DEFAULT_EDITION, root, events, "").await.unwrap();
    }

    // Query for range [2, 4] - inclusive bounds, should return events 2, 3, 4
    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: Some(Selection::Range(SequenceRange {
            lower: 2,
            upper: Some(4),
        })),
    };

    let response = service.get_event_book(Request::new(query)).await;

    assert!(response.is_ok());
    let book = response.unwrap().into_inner();
    assert_eq!(book.pages.len(), 3); // Events 2, 3, 4 (inclusive upper bound)
}

#[tokio::test]
async fn test_get_events_empty_aggregate() {
    let (service, _, _) = create_default_test_service();
    let root = uuid::Uuid::new_v4();

    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_events(Request::new(query)).await;

    assert!(response.is_ok());
    let mut stream = response.unwrap().into_inner();
    let first = stream.next().await;
    assert!(first.is_some());
    let book = first.unwrap().unwrap();
    assert!(book.pages.is_empty());
}

#[tokio::test]
async fn test_get_events_with_data() {
    let (service, event_store, _) = create_default_test_service();
    let root = uuid::Uuid::new_v4();

    // First add some events via the store directly
    let events = vec![EventPage {
        sequence: Some(event_page::Sequence::Num(0)),
        event: Some(Any {
            type_url: "test.Event".to_string(),
            value: vec![],
        }),
        created_at: None,
    }];
    event_store.add("orders", DEFAULT_EDITION, root, events, "").await.unwrap();

    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_events(Request::new(query)).await;

    assert!(response.is_ok());
    let mut stream = response.unwrap().into_inner();
    let first = stream.next().await;
    assert!(first.is_some());
    let book = first.unwrap().unwrap();
    assert_eq!(book.pages.len(), 1);
}

#[tokio::test]
async fn test_get_events_missing_root() {
    let (service, _, _) = create_default_test_service();

    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: None,
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_events(Request::new(query)).await;

    assert!(response.is_err());
    let status = response.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_get_events_invalid_uuid() {
    let (service, _, _) = create_default_test_service();

    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: vec![1, 2, 3], // Invalid: must be 16 bytes
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_events(Request::new(query)).await;

    assert!(response.is_err());
    let status = response.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_get_aggregate_roots_empty() {
    let (service, _, _) = create_default_test_service();

    let response = service.get_aggregate_roots(Request::new(())).await;

    assert!(response.is_ok());
    let mut stream = response.unwrap().into_inner();
    let first = stream.next().await;
    assert!(first.is_none()); // No aggregates yet
}

#[tokio::test]
async fn test_get_aggregate_roots_with_data() {
    let (service, event_store, _) = create_default_test_service();
    let root1 = uuid::Uuid::new_v4();
    let root2 = uuid::Uuid::new_v4();

    // Add some events
    event_store.add("orders", DEFAULT_EDITION, root1, vec![], "").await.unwrap();
    event_store.add("orders", DEFAULT_EDITION, root2, vec![], "").await.unwrap();

    let response = service.get_aggregate_roots(Request::new(())).await;

    assert!(response.is_ok());
    let stream = response.unwrap().into_inner();
    let roots: Vec<_> = stream.collect().await;
    assert_eq!(roots.len(), 2);
}

#[tokio::test]
async fn test_get_aggregate_roots_multiple_domains() {
    let (service, event_store, _) = create_default_test_service();

    event_store
        .add("orders", DEFAULT_EDITION, uuid::Uuid::new_v4(), vec![], "")
        .await
        .unwrap();
    event_store
        .add("inventory", DEFAULT_EDITION, uuid::Uuid::new_v4(), vec![], "")
        .await
        .unwrap();

    let response = service.get_aggregate_roots(Request::new(())).await;

    assert!(response.is_ok());
    let stream = response.unwrap().into_inner();
    let roots: Vec<_> = stream.collect().await;
    assert_eq!(roots.len(), 2);
}

#[tokio::test]
async fn test_get_event_book_by_correlation_id() {
    let (service, event_store, _) = create_default_test_service();
    let root = uuid::Uuid::new_v4();
    let correlation_id = "corr-123";

    // Add events with correlation ID
    let events = vec![EventPage {
        sequence: Some(event_page::Sequence::Num(0)),
        event: Some(Any {
            type_url: "test.Event".to_string(),
            value: vec![],
        }),
        created_at: None,
    }];
    event_store
        .add("orders", DEFAULT_EDITION, root, events, correlation_id)
        .await
        .unwrap();

    // Query by correlation ID (no root needed)
    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: String::new(),
            root: None,
            correlation_id: correlation_id.to_string(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_event_book(Request::new(query)).await;

    assert!(response.is_ok());
    let book = response.unwrap().into_inner();
    assert_eq!(book.pages.len(), 1);
}

#[tokio::test]
async fn test_get_event_book_by_correlation_id_not_found() {
    let (service, _, _) = create_default_test_service();

    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: String::new(),
            root: None,
            correlation_id: "nonexistent".to_string(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_event_book(Request::new(query)).await;

    assert!(response.is_ok());
    let book = response.unwrap().into_inner();
    assert!(book.pages.is_empty());
}

#[tokio::test]
async fn test_get_events_by_correlation_id_multiple_aggregates() {
    let (service, event_store, _) = create_default_test_service();
    let correlation_id = "corr-multi";

    // Add events to multiple aggregates with same correlation ID
    for (domain, root) in [
        ("orders", uuid::Uuid::new_v4()),
        ("inventory", uuid::Uuid::new_v4()),
    ] {
        let events = vec![EventPage {
            sequence: Some(event_page::Sequence::Num(0)),
            event: Some(Any {
                type_url: format!("{}.Event", domain),
                value: vec![],
            }),
            created_at: None,
        }];
        event_store
            .add(domain, DEFAULT_EDITION, root, events, correlation_id)
            .await
            .unwrap();
    }

    // Query by correlation ID - should return both
    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: String::new(),
            root: None,
            correlation_id: correlation_id.to_string(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_events(Request::new(query)).await;

    assert!(response.is_ok());
    let stream = response.unwrap().into_inner();
    let books: Vec<_> = stream.collect().await;
    assert_eq!(books.len(), 2);
}

#[tokio::test]
async fn test_get_event_book_temporal_by_time() {
    let (service, event_store, _) = create_default_test_service();
    let root = uuid::Uuid::new_v4();

    let events = vec![
        EventPage {
            sequence: Some(event_page::Sequence::Num(0)),
            event: Some(Any {
                type_url: "test.Event0".to_string(),
                value: vec![],
            }),
            created_at: Some(Timestamp {
                seconds: 1704067200, // 2024-01-01T00:00:00Z
                nanos: 0,
            }),
        },
        EventPage {
            sequence: Some(event_page::Sequence::Num(1)),
            event: Some(Any {
                type_url: "test.Event1".to_string(),
                value: vec![],
            }),
            created_at: Some(Timestamp {
                seconds: 1704153600, // 2024-01-02T00:00:00Z
                nanos: 0,
            }),
        },
        EventPage {
            sequence: Some(event_page::Sequence::Num(2)),
            event: Some(Any {
                type_url: "test.Event2".to_string(),
                value: vec![],
            }),
            created_at: Some(Timestamp {
                seconds: 1704240000, // 2024-01-03T00:00:00Z
                nanos: 0,
            }),
        },
    ];
    event_store.add("orders", DEFAULT_EDITION, root, events, "").await.unwrap();

    // Query as-of Jan 2
    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: Some(Selection::Temporal(TemporalQuery {
            point_in_time: Some(PointInTime::AsOfTime(Timestamp {
                seconds: 1704153600, // 2024-01-02T00:00:00Z
                nanos: 0,
            })),
        })),
    };

    let response = service.get_event_book(Request::new(query)).await;

    assert!(response.is_ok());
    let book = response.unwrap().into_inner();
    assert_eq!(book.pages.len(), 2); // Events 0 and 1
    assert!(book.snapshot.is_none());
}

#[tokio::test]
async fn test_get_event_book_temporal_by_sequence() {
    let (service, event_store, _) = create_default_test_service();
    let root = uuid::Uuid::new_v4();

    for i in 0..5 {
        let events = vec![EventPage {
            sequence: Some(event_page::Sequence::Num(i)),
            event: Some(Any {
                type_url: format!("test.Event{}", i),
                value: vec![],
            }),
            created_at: None,
        }];
        event_store.add("orders", DEFAULT_EDITION, root, events, "").await.unwrap();
    }

    // Query as-of sequence 2 — should return events 0, 1, 2
    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: Some(Selection::Temporal(TemporalQuery {
            point_in_time: Some(PointInTime::AsOfSequence(2)),
        })),
    };

    let response = service.get_event_book(Request::new(query)).await;

    assert!(response.is_ok());
    let book = response.unwrap().into_inner();
    assert_eq!(book.pages.len(), 3);
    assert!(book.snapshot.is_none());
}

#[tokio::test]
async fn test_get_event_book_temporal_empty_point_in_time() {
    let (service, _, _) = create_default_test_service();
    let root = uuid::Uuid::new_v4();

    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: Some(Selection::Temporal(TemporalQuery {
            point_in_time: None,
        })),
    };

    let response = service.get_event_book(Request::new(query)).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_get_event_book_returns_all_events_despite_snapshot() {
    let (service, event_store, snapshot_store) = create_default_test_service();
    let root = uuid::Uuid::new_v4();

    // Add an event at sequence 0
    let events = vec![EventPage {
        sequence: Some(event_page::Sequence::Num(0)),
        event: Some(Any {
            type_url: "test.CustomerCreated".to_string(),
            value: vec![],
        }),
        created_at: None,
    }];
    event_store.add("customer", DEFAULT_EDITION, root, events, "").await.unwrap();

    // Store a snapshot at sequence 0 (as the aggregate coordinator would)
    let snapshot = crate::proto::Snapshot {
        sequence: 0,
        state: Some(Any {
            type_url: "test.CustomerState".to_string(),
            value: vec![1, 2, 3],
        }),
    };
    snapshot_store.put("customer", DEFAULT_EDITION, root, snapshot).await.unwrap();

    // Query should return the event despite snapshot existing at same sequence
    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: "customer".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };

    let response = service.get_event_book(Request::new(query)).await;

    assert!(response.is_ok());
    let book = response.unwrap().into_inner();
    assert_eq!(book.pages.len(), 1, "EventQuery must return all events regardless of snapshots");
    assert!(book.snapshot.is_none(), "EventQuery should not include snapshots");
}
