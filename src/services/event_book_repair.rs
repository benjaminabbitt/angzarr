//! EventBook completeness detection and repair.
//!
//! Provides utilities for detecting incomplete EventBooks and fetching
//! complete history from the EventQuery service.

use tokio_stream::StreamExt;
use tonic::transport::Channel;
use tracing::{debug, info};
use uuid::Uuid;

use crate::proto::event_page::Sequence;
use crate::proto::{event_query_client::EventQueryClient, EventBook, Query, Uuid as ProtoUuid};

/// Result type for repair operations.
pub type Result<T> = std::result::Result<T, RepairError>;

/// Errors that can occur during EventBook repair.
#[derive(Debug, thiserror::Error)]
pub enum RepairError {
    #[error("EventBook missing cover")]
    MissingCover,

    #[error("EventBook missing root UUID")]
    MissingRoot,

    #[error("Invalid UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),

    #[error("gRPC error: {0}")]
    Grpc(Box<tonic::Status>),

    #[error("Transport error: {0}")]
    Transport(#[from] tonic::transport::Error),

    #[error("Invalid URI: {0}")]
    InvalidUri(String),

    #[error("No EventBook returned from query")]
    NoEventBookReturned,
}

impl From<tonic::Status> for RepairError {
    fn from(status: tonic::Status) -> Self {
        RepairError::Grpc(Box::new(status))
    }
}

/// Check if an EventBook is complete.
///
/// An EventBook is considered complete if:
/// - It has a snapshot, OR
/// - Its first event has sequence 0
///
/// An empty EventBook (no events, no snapshot) is considered complete
/// for a new aggregate.
pub fn is_complete(book: &EventBook) -> bool {
    // Has snapshot - complete from snapshot onwards
    if book.snapshot.is_some() {
        return true;
    }

    // No events - empty aggregate, considered complete
    if book.pages.is_empty() {
        return true;
    }

    // Check if first event is sequence 0
    if let Some(first_page) = book.pages.first() {
        if let Some(ref seq) = first_page.sequence {
            match seq {
                Sequence::Num(n) => return *n == 0,
                Sequence::Force(_) => return true, // Force sequence is always valid
            }
        }
    }

    false
}

/// Extract domain and root UUID from an EventBook.
pub fn extract_identity(book: &EventBook) -> Result<(String, Uuid)> {
    let cover = book.cover.as_ref().ok_or(RepairError::MissingCover)?;
    let root = cover.root.as_ref().ok_or(RepairError::MissingRoot)?;
    let root_uuid = Uuid::from_slice(&root.value)?;
    Ok((cover.domain.clone(), root_uuid))
}

/// Fetch a complete EventBook from the EventQuery service.
///
/// Makes a synchronous gRPC call to fetch the full event history
/// for the given domain and root.
pub async fn fetch_complete(
    client: &mut EventQueryClient<Channel>,
    domain: &str,
    root: Uuid,
) -> Result<EventBook> {
    let query = Query {
        domain: domain.to_string(),
        root: Some(ProtoUuid {
            value: root.as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: 0,
    };

    let response = client.get_events(query).await?;
    let mut stream = response.into_inner();

    // GetEvents returns a stream with a single EventBook
    if let Some(result) = stream.next().await {
        let book = result?;
        debug!(
            domain = %domain,
            root = %root,
            events = book.pages.len(),
            has_snapshot = book.snapshot.is_some(),
            "Fetched complete EventBook"
        );
        Ok(book)
    } else {
        Err(RepairError::NoEventBookReturned)
    }
}

/// Repair an EventBook if incomplete.
///
/// If the EventBook is incomplete (missing history), fetches the complete
/// EventBook from the EventQuery service. Returns the original book if
/// already complete.
pub async fn repair_if_needed(
    client: &mut EventQueryClient<Channel>,
    book: EventBook,
) -> Result<EventBook> {
    if is_complete(&book) {
        debug!("EventBook is complete, no repair needed");
        return Ok(book);
    }

    let (domain, root) = extract_identity(&book)?;
    info!(
        domain = %domain,
        root = %root,
        "EventBook incomplete, fetching complete history"
    );

    fetch_complete(client, &domain, root).await
}

/// Client wrapper for EventBook repair operations.
///
/// Maintains a connection to the EventQuery service and provides
/// convenient methods for repairing EventBooks.
pub struct EventBookRepairer {
    client: EventQueryClient<Channel>,
}

impl EventBookRepairer {
    /// Create a new repairer connected to the given EventQuery service address.
    pub async fn connect(address: &str) -> Result<Self> {
        let channel = Channel::from_shared(format!("http://{}", address))
            .map_err(|e| RepairError::InvalidUri(e.to_string()))?
            .connect()
            .await?;

        Ok(Self {
            client: EventQueryClient::new(channel),
        })
    }

    /// Create a new repairer from an existing channel.
    pub fn new(channel: Channel) -> Self {
        Self {
            client: EventQueryClient::new(channel),
        }
    }

    /// Repair an EventBook if incomplete.
    pub async fn repair(&mut self, book: EventBook) -> Result<EventBook> {
        repair_if_needed(&mut self.client, book).await
    }

    /// Check if an EventBook is complete.
    pub fn is_complete(&self, book: &EventBook) -> bool {
        is_complete(book)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{event_page, Cover, EventPage, Snapshot};
    use prost_types::Any;

    fn make_event(seq: u32) -> EventPage {
        EventPage {
            sequence: Some(event_page::Sequence::Num(seq)),
            event: Some(Any {
                type_url: format!("test.Event{}", seq),
                value: vec![],
            }),
            created_at: None,
            synchronous: false,
        }
    }

    fn make_cover(domain: &str) -> Cover {
        Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: Uuid::new_v4().as_bytes().to_vec(),
            }),
        }
    }

    #[test]
    fn test_is_complete_empty_book() {
        let book = EventBook {
            cover: Some(make_cover("test")),
            pages: vec![],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        assert!(is_complete(&book));
    }

    #[test]
    fn test_is_complete_with_snapshot() {
        let book = EventBook {
            cover: Some(make_cover("test")),
            pages: vec![make_event(5), make_event(6)],
            snapshot: Some(Snapshot {
                sequence: 5,
                state: None,
            }),
            correlation_id: String::new(),
            snapshot_state: None,
        };

        assert!(is_complete(&book));
    }

    #[test]
    fn test_is_complete_starts_at_zero() {
        let book = EventBook {
            cover: Some(make_cover("test")),
            pages: vec![make_event(0), make_event(1), make_event(2)],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        assert!(is_complete(&book));
    }

    #[test]
    fn test_is_incomplete_missing_history() {
        let book = EventBook {
            cover: Some(make_cover("test")),
            pages: vec![make_event(5), make_event(6)], // Missing 0-4
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        assert!(!is_complete(&book));
    }

    #[test]
    fn test_is_incomplete_starts_at_nonzero() {
        let book = EventBook {
            cover: Some(make_cover("test")),
            pages: vec![make_event(3)], // Missing 0-2
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        assert!(!is_complete(&book));
    }

    #[test]
    fn test_extract_identity_success() {
        let root = Uuid::new_v4();
        let book = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![],
            snapshot: None,
            correlation_id: String::new(),
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
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let result = extract_identity(&book);
        assert!(matches!(result, Err(RepairError::MissingCover)));
    }

    #[test]
    fn test_extract_identity_missing_root() {
        let book = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: None,
            }),
            pages: vec![],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let result = extract_identity(&book);
        assert!(matches!(result, Err(RepairError::MissingRoot)));
    }

    mod grpc_integration {
        use super::*;
        use crate::interfaces::EventStore;
        use crate::proto::event_query_server::EventQueryServer;
        use crate::proto::Snapshot;
        use crate::services::EventQueryService;
        use crate::storage::{SqliteEventStore, SqliteSnapshotStore};
        use prost_types::Timestamp;
        use sqlx::SqlitePool;
        use std::net::SocketAddr;
        use std::sync::Arc;
        use tokio::net::TcpListener;
        use tonic::transport::Server;

        async fn test_pool() -> SqlitePool {
            SqlitePool::connect("sqlite::memory:").await.unwrap()
        }

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
                synchronous: false,
            }
        }

        fn make_cover(domain: &str, root: Uuid) -> Cover {
            Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }
        }

        fn make_event_book(domain: &str, root: Uuid, events: Vec<EventPage>) -> EventBook {
            EventBook {
                cover: Some(make_cover(domain, root)),
                snapshot: None,
                pages: events,
                correlation_id: String::new(),
                snapshot_state: None,
            }
        }

        async fn start_event_query_server(
            event_store: Arc<SqliteEventStore>,
            snapshot_store: Arc<SqliteSnapshotStore>,
        ) -> SocketAddr {
            start_event_query_server_with_options(event_store, snapshot_store, false).await
        }

        async fn start_event_query_server_with_options(
            event_store: Arc<SqliteEventStore>,
            snapshot_store: Arc<SqliteSnapshotStore>,
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
            let pool = test_pool().await;
            let event_store = Arc::new(SqliteEventStore::new(pool.clone()));
            let snapshot_store = Arc::new(SqliteSnapshotStore::new(pool));
            event_store.init().await.unwrap();
            snapshot_store.init().await.unwrap();

            let domain = "orders";
            let root = Uuid::new_v4();
            event_store
                .add(
                    domain,
                    root,
                    vec![
                        test_event(0, "Created"),
                        test_event(1, "Updated"),
                        test_event(2, "ItemAdded"),
                        test_event(3, "ItemAdded"),
                        test_event(4, "Completed"),
                    ],
                )
                .await
                .unwrap();

            let addr = start_event_query_server(event_store, snapshot_store).await;

            let mut repairer = EventBookRepairer::connect(&addr.to_string()).await.unwrap();

            let incomplete_book = make_event_book(domain, root, vec![test_event(4, "Completed")]);
            assert!(!is_complete(&incomplete_book));

            let repaired = repairer.repair(incomplete_book).await.unwrap();

            assert!(is_complete(&repaired));
            assert_eq!(repaired.pages.len(), 5);
            assert_eq!(repaired.pages[0].sequence, Some(Sequence::Num(0)));
            assert_eq!(repaired.pages[4].sequence, Some(Sequence::Num(4)));
        }

        #[tokio::test]
        async fn test_repairer_passes_through_complete_book() {
            let pool = test_pool().await;
            let event_store = Arc::new(SqliteEventStore::new(pool.clone()));
            let snapshot_store = Arc::new(SqliteSnapshotStore::new(pool));
            event_store.init().await.unwrap();
            snapshot_store.init().await.unwrap();

            let addr = start_event_query_server(event_store, snapshot_store).await;

            let mut repairer = EventBookRepairer::connect(&addr.to_string()).await.unwrap();

            let complete_book = make_event_book(
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
            let pool = test_pool().await;
            let event_store = Arc::new(SqliteEventStore::new(pool.clone()));
            let snapshot_store = Arc::new(SqliteSnapshotStore::new(pool));
            event_store.init().await.unwrap();
            snapshot_store.init().await.unwrap();

            let domain = "orders";
            let root = Uuid::new_v4();
            let events: Vec<EventPage> = (0..10)
                .map(|i| test_event(i, &format!("Event{}", i)))
                .collect();
            event_store.add(domain, root, events).await.unwrap();

            use crate::interfaces::SnapshotStore;
            snapshot_store
                .put(
                    domain,
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

            let incomplete_book = make_event_book(
                domain,
                root,
                vec![test_event(8, "Event8"), test_event(9, "Event9")],
            );
            assert!(!is_complete(&incomplete_book));

            let repaired = repairer.repair(incomplete_book).await.unwrap();

            assert!(is_complete(&repaired));
            assert!(repaired.snapshot.is_some());
            assert_eq!(repaired.snapshot.as_ref().unwrap().sequence, 5);
            assert_eq!(repaired.pages.len(), 5);
            assert_eq!(repaired.pages[0].sequence, Some(Sequence::Num(5)));
        }

        #[tokio::test]
        async fn test_repairer_empty_aggregate_returns_empty() {
            let pool = test_pool().await;
            let event_store = Arc::new(SqliteEventStore::new(pool.clone()));
            let snapshot_store = Arc::new(SqliteSnapshotStore::new(pool));
            event_store.init().await.unwrap();
            snapshot_store.init().await.unwrap();

            let addr = start_event_query_server(event_store, snapshot_store).await;

            let mut repairer = EventBookRepairer::connect(&addr.to_string()).await.unwrap();

            let root = Uuid::new_v4();
            let incomplete_book = make_event_book("orders", root, vec![test_event(5, "Event5")]);

            let repaired = repairer.repair(incomplete_book).await.unwrap();

            assert!(is_complete(&repaired));
            assert!(repaired.pages.is_empty());
        }
    }
}
