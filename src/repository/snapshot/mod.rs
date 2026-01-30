//! Snapshot repository.
//!
//! Provides aggregate snapshot persistence operations.

use std::sync::Arc;
use uuid::Uuid;

use crate::proto::Snapshot;
use crate::storage::{Result, SnapshotStore};

/// Repository for Snapshot operations.
///
/// Handles persisting and retrieving aggregate state snapshots.
/// Snapshots are an optimization to avoid replaying entire event history.
pub struct SnapshotRepository {
    store: Arc<dyn SnapshotStore>,
    /// When false, snapshots are not written.
    write_enabled: bool,
}

impl SnapshotRepository {
    /// Create a new Snapshot repository with writes enabled.
    pub fn new(store: Arc<dyn SnapshotStore>) -> Self {
        Self {
            store,
            write_enabled: true,
        }
    }

    /// Create a new Snapshot repository with configurable write behavior.
    pub fn with_config(store: Arc<dyn SnapshotStore>, write_enabled: bool) -> Self {
        Self {
            store,
            write_enabled,
        }
    }

    /// Retrieve the latest snapshot for an aggregate.
    ///
    /// Returns `None` if no snapshot exists.
    pub async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Option<Snapshot>> {
        self.store.get(domain, edition, root).await
    }

    /// Store a snapshot for an aggregate.
    ///
    /// Replaces any existing snapshot for this root.
    /// If writes are disabled, this is a no-op.
    pub async fn put(&self, domain: &str, edition: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        if self.write_enabled {
            self.store.put(domain, edition, root, snapshot).await
        } else {
            Ok(())
        }
    }

    /// Delete the snapshot for an aggregate.
    pub async fn delete(&self, domain: &str, edition: &str, root: Uuid) -> Result<()> {
        self.store.delete(domain, edition, root).await
    }
}

#[cfg(test)]
mod tests;
