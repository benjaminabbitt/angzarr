//! EventBook repository.
//!
//! Combines event store and snapshot store to provide
//! aggregate-level event book operations.

use std::sync::Arc;
use uuid::Uuid;

use crate::proto::{Cover, Edition, EventBook, Uuid as ProtoUuid};
use crate::storage::{EventStore, Result, SnapshotStore, StorageError};

/// Extract domain, root UUID, and correlation_id from an EventBook.
fn extract_cover(book: &EventBook) -> Result<(&str, Uuid, &str)> {
    let cover = book.cover.as_ref().ok_or(StorageError::MissingCover)?;
    let root = cover.root.as_ref().ok_or(StorageError::MissingRoot)?;
    let root_uuid = Uuid::from_slice(&root.value)?;
    Ok((&cover.domain, root_uuid, &cover.correlation_id))
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
    pub async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<EventBook> {
        // Try to load snapshot (only if snapshot reading is enabled)
        let snapshot = if self.snapshot_read_enabled {
            self.snapshot_store.get(domain, edition, root).await?
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
            .get_from(domain, edition, root, from_sequence)
            .await?;

        Ok(EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: Some(Edition { name: edition.to_string(), divergences: vec![] }),
            }),
            snapshot,
            pages: events,
        })
    }

    /// Load an EventBook with events in a specific range.
    pub async fn get_from_to(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<EventBook> {
        let events = self.event_store.get_from_to(domain, edition, root, from, to).await?;

        Ok(EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: Some(Edition { name: edition.to_string(), divergences: vec![] }),
            }),
            snapshot: None,
            pages: events,
        })
    }

    /// Load an EventBook as-of a timestamp (no snapshots).
    ///
    /// Returns events from sequence 0 with created_at <= until.
    /// Snapshots are skipped to ensure correct historical reconstruction.
    pub async fn get_temporal_by_time(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        until: &str,
    ) -> Result<EventBook> {
        let events = self
            .event_store
            .get_until_timestamp(domain, edition, root, until)
            .await?;

        Ok(EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: Some(Edition { name: edition.to_string(), divergences: vec![] }),
            }),
            snapshot: None,
            pages: events,
        })
    }

    /// Load an EventBook as-of a sequence number (no snapshots).
    ///
    /// Returns events from sequence 0 through `sequence` inclusive.
    /// Snapshots are skipped to ensure correct historical reconstruction.
    pub async fn get_temporal_by_sequence(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        sequence: u32,
    ) -> Result<EventBook> {
        let events = self
            .event_store
            .get_from_to(domain, edition, root, 0, sequence.saturating_add(1))
            .await?;

        Ok(EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: Some(Edition { name: edition.to_string(), divergences: vec![] }),
            }),
            snapshot: None,
            pages: events,
        })
    }

    /// Persist an EventBook.
    ///
    /// Stores all events in the event store.
    pub async fn put(&self, edition: &str, book: &EventBook) -> Result<()> {
        let (domain, root_uuid, correlation_id) = extract_cover(book)?;
        self.event_store
            .add(domain, edition, root_uuid, book.pages.clone(), correlation_id)
            .await
    }
}

#[cfg(test)]
mod tests;
