//! Bigtable SnapshotStore implementation (placeholder).

use async_trait::async_trait;
use uuid::Uuid;

use crate::proto::Snapshot;
use crate::storage::{Result, SnapshotStore};

/// Bigtable implementation of SnapshotStore.
pub struct BigtableSnapshotStore;

#[async_trait]
impl SnapshotStore for BigtableSnapshotStore {
    async fn get(&self, _domain: &str, _root: Uuid) -> Result<Option<Snapshot>> {
        todo!("BigtableSnapshotStore::get")
    }

    async fn put(&self, _domain: &str, _root: Uuid, _snapshot: Snapshot) -> Result<()> {
        todo!("BigtableSnapshotStore::put")
    }

    async fn delete(&self, _domain: &str, _root: Uuid) -> Result<()> {
        todo!("BigtableSnapshotStore::delete")
    }
}
