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
