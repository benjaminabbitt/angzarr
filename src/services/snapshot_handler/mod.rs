//! Snapshot handling logic for AggregateService.
//!
//! Handles snapshot loading, state extraction from EventBook, and persistence.

use std::sync::Arc;

use tonic::Status;
use uuid::Uuid;

use crate::proto::{event_page, EventBook, Snapshot, SnapshotRetention};
use crate::storage::SnapshotStore;

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

/// Persists a snapshot if the EventBook contains new events and a snapshot with state.
///
/// Only persists when:
/// - write_enabled is true
/// - pages is non-empty (there are new events to snapshot)
/// - snapshot.state is Some (client explicitly provided state to snapshot)
///
/// The snapshot's sequence is computed from the last event in pages, not from
/// the snapshot.sequence field (which may be stale from loading).
///
/// # Arguments
/// * `snapshot_store` - The storage backend for snapshots
/// * `event_book` - The EventBook potentially containing a snapshot to persist
/// * `domain` - The domain name for the aggregate
/// * `edition` - The edition identifier for multi-tenant partitioning
/// * `root_uuid` - The aggregate root UUID
/// * `write_enabled` - Whether snapshot writing is enabled
///
/// # Returns
/// Ok(()) on success, or a Status error if persistence fails
pub async fn persist_snapshot_if_present(
    snapshot_store: &Arc<dyn SnapshotStore>,
    event_book: &EventBook,
    domain: &str,
    edition: &str,
    root_uuid: Uuid,
    write_enabled: bool,
) -> Result<(), Status> {
    if !write_enabled {
        return Ok(());
    }

    // Only persist if there are new events AND snapshot state is provided
    if event_book.pages.is_empty() {
        return Ok(());
    }

    if let Some(ref snapshot) = event_book.snapshot {
        if let Some(ref state) = snapshot.state {
            // Compute sequence from the last event being persisted
            let snapshot_sequence = compute_snapshot_sequence(event_book);
            let persisted_snapshot = Snapshot {
                sequence: snapshot_sequence,
                state: Some(state.clone()),
                retention: SnapshotRetention::RetentionDefault as i32,
            };
            snapshot_store
                .put(domain, edition, root_uuid, persisted_snapshot)
                .await
                .map_err(|e| Status::internal(format!("Failed to persist snapshot: {e}")))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
