//! EventBook repository.
//!
//! Combines event store and snapshot store to provide
//! aggregate-level event book operations.

use std::sync::Arc;
use uuid::Uuid;

use crate::storage::{EventStore, Result, SnapshotStore, StorageError};
use crate::proto::{Cover, EventBook, Uuid as ProtoUuid};

/// Extract domain and root UUID from an EventBook.
fn extract_cover(book: &EventBook) -> Result<(&str, Uuid)> {
    let cover = book.cover.as_ref().ok_or(StorageError::MissingCover)?;
    let root = cover.root.as_ref().ok_or(StorageError::MissingRoot)?;
    let root_uuid = Uuid::from_slice(&root.value)?;
    Ok((&cover.domain, root_uuid))
}

/// Repository for EventBook operations.
///
/// Handles loading aggregates with snapshot optimization
/// and persisting new events.
pub struct EventBookRepository {
    event_store: Arc<dyn EventStore>,
    snapshot_store: Arc<dyn SnapshotStore>,
    /// When false, snapshots are not loaded; all events are replayed from the beginning.
    snapshot_read_enabled: bool,
}

impl EventBookRepository {
    /// Create a new EventBook repository with snapshots enabled.
    pub fn new(event_store: Arc<dyn EventStore>, snapshot_store: Arc<dyn SnapshotStore>) -> Self {
        Self {
            event_store,
            snapshot_store,
            snapshot_read_enabled: true,
        }
    }

    /// Create a new EventBook repository with configurable snapshot reading.
    pub fn with_config(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        snapshot_read_enabled: bool,
    ) -> Self {
        Self {
            event_store,
            snapshot_store,
            snapshot_read_enabled,
        }
    }

    /// Load an EventBook for an aggregate.
    ///
    /// If snapshot reading is enabled and a snapshot exists, loads events
    /// from the snapshot sequence. Otherwise, loads all events from the beginning.
    pub async fn get(&self, domain: &str, root: Uuid) -> Result<EventBook> {
        // Try to load snapshot (only if snapshot reading is enabled)
        let snapshot = if self.snapshot_read_enabled {
            self.snapshot_store.get(domain, root).await?
        } else {
            None
        };

        // Determine starting sequence
        // Snapshot sequence is the last event sequence used to create the snapshot,
        // so we start loading from snapshot.sequence + 1 to avoid double-applying events
        let from_sequence = snapshot.as_ref().map(|s| s.sequence + 1).unwrap_or(0);

        // Load events after snapshot (or from beginning if no snapshot)
        let events = self
            .event_store
            .get_from(domain, root, from_sequence)
            .await?;

        Ok(EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            snapshot,
            pages: events,
            correlation_id: String::new(),
            snapshot_state: None,
        })
    }

    /// Load an EventBook with events in a specific range.
    pub async fn get_from_to(
        &self,
        domain: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<EventBook> {
        let events = self.event_store.get_from_to(domain, root, from, to).await?;

        Ok(EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            snapshot: None,
            pages: events,
            correlation_id: String::new(),
            snapshot_state: None,
        })
    }

    /// Persist an EventBook.
    ///
    /// Stores all events in the event store.
    pub async fn put(&self, book: &EventBook) -> Result<()> {
        let (domain, root_uuid) = extract_cover(book)?;
        self.event_store
            .add(domain, root_uuid, book.pages.clone())
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{event_page, EventPage, Snapshot};
    use crate::storage::mock::{MockEventStore, MockSnapshotStore};
    use prost_types::Any;

    fn make_event(seq: u32) -> EventPage {
        EventPage {
            sequence: Some(event_page::Sequence::Num(seq)),
            event: Some(Any {
                type_url: format!("test.Event{}", seq),
                value: vec![],
            }),
            created_at: None,
        }
    }

    #[tokio::test]
    async fn test_get_returns_empty_book_for_new_aggregate() {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());
        let repo = EventBookRepository::new(event_store, snapshot_store);

        let root = Uuid::new_v4();
        let book = repo.get("orders", root).await.unwrap();

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
        let book = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![make_event(0), make_event(1)],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        repo.put(&book).await.unwrap();

        let retrieved = repo.get("orders", root).await.unwrap();
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
            .add("orders", root, (0..5).map(make_event).collect())
            .await
            .unwrap();

        // Add snapshot at sequence 3
        snapshot_store
            .put(
                "orders",
                root,
                Snapshot {
                    sequence: 3,
                    state: None,
                },
            )
            .await
            .unwrap();

        let book = repo.get("orders", root).await.unwrap();

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
            .add("orders", root, (0..10).map(make_event).collect())
            .await
            .unwrap();

        let book = repo.get_from_to("orders", root, 3, 7).await.unwrap();

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
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let result = repo.put(&book).await;

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
            }),
            pages: vec![],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let result = repo.put(&book).await;

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
            }),
            pages: vec![],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let result = repo.put(&book).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_propagates_store_error() {
        let event_store = Arc::new(MockEventStore::new());
        event_store.set_fail_on_get(true).await;
        let snapshot_store = Arc::new(MockSnapshotStore::new());
        let repo = EventBookRepository::new(event_store, snapshot_store);

        let result = repo.get("orders", Uuid::new_v4()).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_put_propagates_store_error() {
        let event_store = Arc::new(MockEventStore::new());
        event_store.set_fail_on_add(true).await;
        let snapshot_store = Arc::new(MockSnapshotStore::new());
        let repo = EventBookRepository::new(event_store, snapshot_store);

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

        let result = repo.put(&book).await;

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
            .add("orders", root, (0..5).map(make_event).collect())
            .await
            .unwrap();

        // Add snapshot at sequence 3
        snapshot_store
            .put(
                "orders",
                root,
                Snapshot {
                    sequence: 3,
                    state: None,
                },
            )
            .await
            .unwrap();

        let book = repo.get("orders", root).await.unwrap();

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
            .add("orders", root, (0..5).map(make_event).collect())
            .await
            .unwrap();

        // Add snapshot at sequence 3
        snapshot_store
            .put(
                "orders",
                root,
                Snapshot {
                    sequence: 3,
                    state: None,
                },
            )
            .await
            .unwrap();

        let book = repo.get("orders", root).await.unwrap();

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
            .add("orders", root, (0..3).map(make_event).collect())
            .await
            .unwrap();

        snapshot_store
            .put(
                "orders",
                root,
                Snapshot {
                    sequence: 2,
                    state: None,
                },
            )
            .await
            .unwrap();

        let book_new = repo_new.get("orders", root).await.unwrap();
        let book_config = repo_config.get("orders", root).await.unwrap();

        assert_eq!(book_new.pages.len(), book_config.pages.len());
        assert_eq!(book_new.snapshot.is_some(), book_config.snapshot.is_some());
    }

    mod mock_integration {
        use super::*;
        use crate::storage::mock::{MockEventStore, MockSnapshotStore};
        use prost_types::Timestamp;

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
                correlation_id: String::new(),
                snapshot_state: None,
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

            let book = repo.get(domain, root).await.unwrap();

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

            let book = make_event_book(domain, root, events);
            repo.put(&book).await.unwrap();

            let retrieved = repo.get(domain, root).await.unwrap();
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

            use crate::storage::SnapshotStore;
            snapshot_store
                .put(domain, root, test_snapshot(3))
                .await
                .unwrap();

            let book = repo.get(domain, root).await.unwrap();

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

            let book1 = make_event_book(domain, root, vec![test_event(0, "Created")]);
            repo.put(&book1).await.unwrap();

            let book2 = make_event_book(domain, root, vec![test_event(1, "Updated")]);
            repo.put(&book2).await.unwrap();

            let retrieved = repo.get(domain, root).await.unwrap();
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

            use crate::storage::SnapshotStore;
            snapshot_store
                .put(domain, root, test_snapshot(3))
                .await
                .unwrap();

            let book = repo.get(domain, root).await.unwrap();

            assert!(book.snapshot.is_none());
            assert_eq!(book.pages.len(), 5);
            assert_eq!(book.pages[0].sequence, Some(event_page::Sequence::Num(0)));
            assert_eq!(book.pages[4].sequence, Some(event_page::Sequence::Num(4)));
        }
    }
}
