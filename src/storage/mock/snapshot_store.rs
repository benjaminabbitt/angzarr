//! Mock SnapshotStore implementation for testing.

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::proto::Snapshot;
use crate::storage::{Result, SnapshotStore};

/// Mock snapshot store that stores snapshots in memory.
#[derive(Default)]
pub struct MockSnapshotStore {
    snapshots: RwLock<HashMap<(String, String, Uuid), Snapshot>>,
}

impl MockSnapshotStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_stored(&self, domain: &str, edition: &str, root: Uuid) -> Option<Snapshot> {
        let key = (domain.to_string(), edition.to_string(), root);
        self.snapshots.read().await.get(&key).cloned()
    }

    pub async fn stored_count(&self) -> usize {
        self.snapshots.read().await.len()
    }
}

#[async_trait]
impl SnapshotStore for MockSnapshotStore {
    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let key = (domain.to_string(), edition.to_string(), root);
        let store = self.snapshots.read().await;
        Ok(store.get(&key).cloned())
    }

    async fn put(&self, domain: &str, edition: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let key = (domain.to_string(), edition.to_string(), root);
        self.snapshots.write().await.insert(key, snapshot);
        Ok(())
    }

    async fn delete(&self, domain: &str, edition: &str, root: Uuid) -> Result<()> {
        let key = (domain.to_string(), edition.to_string(), root);
        self.snapshots.write().await.remove(&key);
        Ok(())
    }
}
