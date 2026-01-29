//! Snapshot handling logic for AggregateService.
//!
//! Handles snapshot loading, state extraction from EventBook, and persistence.

use std::sync::Arc;

use tonic::Status;
use uuid::Uuid;

use crate::proto::{event_page, EventBook, Snapshot};
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
mod tests;
