//! Snapshot handling logic for AggregateService.
//!
//! Handles snapshot loading, state extraction from EventBook, and persistence.

use std::sync::Arc;

use tonic::Status;
use uuid::Uuid;

use crate::storage::SnapshotStore;
use crate::proto::{event_page, EventBook, Snapshot};

/// Computes the snapshot sequence from the last event in an EventBook.
///
/// The snapshot sequence is the sequence number of the last event used
/// to create this snapshot (not incremented).
pub fn compute_snapshot_sequence(event_book: &EventBook) -> u32 {
    event_book
        .pages
        .last()
        .and_then(|p| match &p.sequence {
            Some(event_page::Sequence::Num(n)) => Some(*n),
            _ => None,
        })
        .unwrap_or(0)
}

/// Persists a snapshot if the EventBook contains snapshot state and writing is enabled.
///
/// # Arguments
/// * `snapshot_store` - The storage backend for snapshots
/// * `event_book` - The EventBook potentially containing snapshot state
/// * `domain` - The domain name for the aggregate
/// * `root_uuid` - The aggregate root UUID
/// * `write_enabled` - Whether snapshot writing is enabled
///
/// # Returns
/// Ok(()) on success, or a Status error if persistence fails
pub async fn persist_snapshot_if_present(
    snapshot_store: &Arc<dyn SnapshotStore>,
    event_book: &EventBook,
    domain: &str,
    root_uuid: Uuid,
    write_enabled: bool,
) -> Result<(), Status> {
    if !write_enabled {
        return Ok(());
    }

    if let Some(ref state) = event_book.snapshot_state {
        let snapshot_sequence = compute_snapshot_sequence(event_book);
        let snapshot = Snapshot {
            sequence: snapshot_sequence,
            state: Some(state.clone()),
        };
        snapshot_store
            .put(domain, root_uuid, snapshot)
            .await
            .map_err(|e| Status::internal(format!("Failed to persist snapshot: {e}")))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Cover, EventPage, Uuid as ProtoUuid};
    use crate::storage::mock::MockSnapshotStore;
    use prost_types::Any;

    fn make_event_page(sequence: u32) -> EventPage {
        EventPage {
            sequence: Some(event_page::Sequence::Num(sequence)),
            event: Some(Any {
                type_url: "test.Event".to_string(),
                value: vec![],
            }),
            created_at: None,
        }
    }

    fn make_event_book_with_snapshot(pages: Vec<EventPage>, has_snapshot: bool) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid {
                    value: Uuid::new_v4().as_bytes().to_vec(),
                }),
            }),
            pages,
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: if has_snapshot {
                Some(Any {
                    type_url: "test.State".to_string(),
                    value: vec![1, 2, 3],
                })
            } else {
                None
            },
        }
    }

    #[test]
    fn test_compute_snapshot_sequence_empty_pages() {
        let event_book = make_event_book_with_snapshot(vec![], false);
        assert_eq!(compute_snapshot_sequence(&event_book), 0);
    }

    #[test]
    fn test_compute_snapshot_sequence_single_page() {
        let event_book = make_event_book_with_snapshot(vec![make_event_page(0)], false);
        assert_eq!(compute_snapshot_sequence(&event_book), 0);
    }

    #[test]
    fn test_compute_snapshot_sequence_multiple_pages() {
        let event_book = make_event_book_with_snapshot(
            vec![make_event_page(0), make_event_page(1), make_event_page(2)],
            false,
        );
        assert_eq!(compute_snapshot_sequence(&event_book), 2);
    }

    #[tokio::test]
    async fn test_persist_snapshot_if_present_disabled() {
        let snapshot_store: Arc<dyn SnapshotStore> = Arc::new(MockSnapshotStore::new());
        let mock_store = Arc::new(MockSnapshotStore::new());
        let event_book = make_event_book_with_snapshot(vec![make_event_page(0)], true);
        let root = Uuid::new_v4();

        let result =
            persist_snapshot_if_present(&snapshot_store, &event_book, "test", root, false).await;

        assert!(result.is_ok());
        // No snapshot should be stored when disabled
        let stored = mock_store.get_stored("test", root).await;
        assert!(stored.is_none());
    }

    #[tokio::test]
    async fn test_persist_snapshot_if_present_no_state() {
        let mock_store = Arc::new(MockSnapshotStore::new());
        let snapshot_store: Arc<dyn SnapshotStore> =
            Arc::clone(&mock_store) as Arc<dyn SnapshotStore>;
        let event_book = make_event_book_with_snapshot(vec![make_event_page(0)], false);
        let root = Uuid::new_v4();

        let result =
            persist_snapshot_if_present(&snapshot_store, &event_book, "test", root, true).await;

        assert!(result.is_ok());
        // No snapshot should be stored when no state
        let stored = mock_store.get_stored("test", root).await;
        assert!(stored.is_none());
    }

    #[tokio::test]
    async fn test_persist_snapshot_if_present_success() {
        let mock_store = Arc::new(MockSnapshotStore::new());
        let snapshot_store: Arc<dyn SnapshotStore> =
            Arc::clone(&mock_store) as Arc<dyn SnapshotStore>;
        let event_book = make_event_book_with_snapshot(vec![make_event_page(0)], true);
        let root = Uuid::new_v4();

        let result =
            persist_snapshot_if_present(&snapshot_store, &event_book, "test", root, true).await;

        assert!(result.is_ok());
        let stored = mock_store.get_stored("test", root).await;
        assert!(stored.is_some());
        // Snapshot sequence is the last event sequence (0), not incremented
        assert_eq!(stored.unwrap().sequence, 0);
    }
}
