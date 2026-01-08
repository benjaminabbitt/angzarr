//! EventBook repository.
//!
//! Combines event store and snapshot store to provide
//! aggregate-level event book operations.

use std::sync::Arc;
use uuid::Uuid;

use crate::interfaces::event_store::{EventStore, Result, StorageError};
use crate::interfaces::SnapshotStore;
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
}

impl EventBookRepository {
    /// Create a new EventBook repository.
    pub fn new(event_store: Arc<dyn EventStore>, snapshot_store: Arc<dyn SnapshotStore>) -> Self {
        Self {
            event_store,
            snapshot_store,
        }
    }

    /// Load an EventBook for an aggregate.
    ///
    /// If a snapshot exists, loads events from the snapshot sequence.
    /// Otherwise, loads all events from the beginning.
    pub async fn get(&self, domain: &str, root: Uuid) -> Result<EventBook> {
        // Try to load snapshot
        let snapshot = self.snapshot_store.get(domain, root).await?;

        // Determine starting sequence
        let from_sequence = snapshot.as_ref().map(|s| s.sequence).unwrap_or(0);

        // Load events from snapshot onwards
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
    use crate::test_utils::{MockEventStore, MockSnapshotStore};
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
            .add(
                "orders",
                root,
                (0..5).map(make_event).collect(),
            )
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

        // Should only have events from sequence 3 onwards
        assert_eq!(book.pages.len(), 2); // Events 3 and 4
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
        };

        let result = repo.put(&book).await;

        assert!(result.is_err());
    }
}
