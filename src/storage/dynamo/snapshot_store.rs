//! DynamoDB SnapshotStore implementation (placeholder).

use async_trait::async_trait;
use uuid::Uuid;

use crate::proto::Snapshot;
use crate::storage::{Result, SnapshotStore};

/// DynamoDB implementation of SnapshotStore.
pub struct DynamoSnapshotStore;

#[async_trait]
impl SnapshotStore for DynamoSnapshotStore {
    async fn get(&self, _domain: &str, _root: Uuid) -> Result<Option<Snapshot>> {
        todo!("DynamoSnapshotStore::get")
    }

    async fn put(&self, _domain: &str, _root: Uuid, _snapshot: Snapshot) -> Result<()> {
        todo!("DynamoSnapshotStore::put")
    }

    async fn delete(&self, _domain: &str, _root: Uuid) -> Result<()> {
        todo!("DynamoSnapshotStore::delete")
    }
}
