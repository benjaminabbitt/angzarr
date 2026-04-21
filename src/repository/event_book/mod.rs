//! EventBook repository.
//!
//! Combines event store and snapshot store to provide aggregate-level event
//! book operations with snapshot optimization.
//!
//! # Why This Repository Exists
//!
//! Event sourcing faces a fundamental tension: events are append-only and
//! immutable (great for audit), but rebuilding state from the beginning is
//! O(n) in event count (bad for performance). Snapshots solve this by
//! periodically capturing materialized state, allowing recovery from
//! snapshot + subsequent events instead of all events.
//!
//! This repository encapsulates the snapshot-loading strategy:
//!
//! 1. Load snapshot (if enabled and exists)
//! 2. Fetch only events AFTER the snapshot's sequence
//! 3. Combine into an EventBook that the caller can replay
//!
//! The caller doesn't need to know whether state came from 3 events or
//! 3 million events with a snapshot — the EventBook looks the same.
//!
//! # Snapshot Sequence Semantics
//!
//! A snapshot's `sequence` field is the sequence number of the LAST event
//! that was included when creating the snapshot. When loading, we fetch
//! events starting from `snapshot.sequence + 1` to avoid double-applying
//! the event that's already baked into the snapshot state.

use std::sync::Arc;
use uuid::Uuid;

use std::collections::HashSet;

use crate::proto::{Cover, Edition, EventBook, Uuid as ProtoUuid};
use crate::proto_ext::{calculate_set_next_seq, EventPageExt};
use crate::storage::{AddOutcome, EventStore, Result, SnapshotStore, StorageError};

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
    ///
    /// # Why Disable Snapshot Reading?
    ///
    /// Several scenarios require full event replay:
    ///
    /// 1. **State migration**: When the snapshot format changes (new fields, renamed
    ///    fields, structural changes), old snapshots may be incompatible. Disabling
    ///    snapshot reads forces full replay through the new event handlers.
    ///
    /// 2. **Debugging/auditing**: To verify that snapshot creation is correct, compare
    ///    state-from-snapshot vs state-from-full-replay. They should be identical.
    ///
    /// 3. **Snapshot regeneration**: After a bug fix in event application logic, old
    ///    snapshots contain incorrect state. Replay from scratch to regenerate them.
    ///
    /// 4. **Testing**: Unit tests may want to exercise the full replay path to ensure
    ///    event handlers are correct, independent of snapshot machinery.
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

        let mut book = EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: Some(Edition {
                    name: edition.to_string(),
                    divergences: vec![],
                }),
            }),
            snapshot,
            pages: events,
            ..Default::default()
        };
        calculate_set_next_seq(&mut book);
        Ok(book)
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
        let events = self
            .event_store
            .get_from_to(domain, edition, root, from, to)
            .await?;

        let mut book = EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: Some(Edition {
                    name: edition.to_string(),
                    divergences: vec![],
                }),
            }),
            snapshot: None,
            pages: events,
            ..Default::default()
        };
        calculate_set_next_seq(&mut book);
        Ok(book)
    }

    /// Load an EventBook as-of a timestamp (no snapshots).
    ///
    /// Returns events from sequence 0 with created_at <= until.
    ///
    /// # Why Temporal Queries Skip Snapshots
    ///
    /// Snapshots represent state at a SPECIFIC point in time — the moment they were
    /// created. If we're querying "what was the state at time T", we can't use a
    /// snapshot created at time T+5 (it would include future events) nor one at T-10
    /// (we'd still need to replay events from T-10 to T, negating the benefit).
    ///
    /// The only correct approach is replaying all events from the beginning through
    /// the requested point in time. This is O(n) but temporal queries are typically
    /// for debugging, auditing, or occasional analytics — not hot paths.
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

        let mut book = EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: Some(Edition {
                    name: edition.to_string(),
                    divergences: vec![],
                }),
            }),
            snapshot: None,
            pages: events,
            ..Default::default()
        };
        calculate_set_next_seq(&mut book);
        Ok(book)
    }

    /// Load an EventBook as-of a sequence number (no snapshots).
    ///
    /// Returns events from sequence 0 through `sequence` inclusive.
    ///
    /// # Why Sequence-Based Temporal Queries Skip Snapshots
    ///
    /// Same reasoning as timestamp-based queries: snapshots capture state at a
    /// specific sequence, and using the wrong snapshot would produce incorrect
    /// historical state. Full replay ensures correctness.
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

        let mut book = EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: Some(Edition {
                    name: edition.to_string(),
                    divergences: vec![],
                }),
            }),
            snapshot: None,
            pages: events,
            ..Default::default()
        };
        calculate_set_next_seq(&mut book);
        Ok(book)
    }

    /// Load an EventBook with only specific sequences.
    ///
    /// Returns events matching the requested sequence numbers.
    /// Useful for sparse queries where only certain events are needed.
    ///
    /// # Performance Note
    ///
    /// Currently fetches all events and filters in memory. For aggregates
    /// with many events, consider adding `get_sequences` to `EventStore`
    /// trait for database-level filtering.
    pub async fn get_sequences(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        sequences: &[u32],
    ) -> Result<EventBook> {
        // Convert to HashSet for O(1) lookup
        let seq_set: HashSet<u32> = sequences.iter().copied().collect();

        // Optimization: if sequences are contiguous, use range query
        if !sequences.is_empty() {
            let min_seq = *sequences.iter().min().unwrap();
            let max_seq = *sequences.iter().max().unwrap();
            let range_size = (max_seq - min_seq + 1) as usize;

            // If requested sequences span a contiguous range, use range query
            if range_size == sequences.len() {
                return self
                    .get_from_to(domain, edition, root, min_seq, max_seq + 1)
                    .await;
            }
        }

        // Sparse sequences: fetch all and filter
        let all_events = self.event_store.get(domain, edition, root).await?;

        let filtered_events: Vec<_> = all_events
            .into_iter()
            .filter(|page| seq_set.contains(&page.sequence_num()))
            .collect();

        let mut book = EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: Some(Edition {
                    name: edition.to_string(),
                    divergences: vec![],
                }),
            }),
            snapshot: None,
            pages: filtered_events,
            ..Default::default()
        };
        calculate_set_next_seq(&mut book);
        Ok(book)
    }

    /// Persist an EventBook.
    ///
    /// Stores all events in the event store. When `external_id` is provided,
    /// the storage layer atomically checks for duplicates. When
    /// `source_info` is provided (saga-produced commands), the storage
    /// layer tags each persisted event with that provenance for the
    /// `find_by_source` deferred-idempotency lookup.
    pub async fn put(
        &self,
        edition: &str,
        book: &EventBook,
        external_id: Option<&str>,
        source_info: Option<&crate::storage::SourceInfo>,
    ) -> Result<AddOutcome> {
        let (domain, root_uuid, correlation_id) = extract_cover(book)?;
        self.event_store
            .add(
                domain,
                edition,
                root_uuid,
                book.pages.clone(),
                correlation_id,
                external_id,
                source_info,
            )
            .await
    }
}

#[cfg(test)]
#[path = "mod.test.rs"]
mod tests;
