//! Redis SnapshotStore implementation.

use async_trait::async_trait;
use prost::Message;
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use tracing::{debug, info};
use uuid::Uuid;

use crate::proto::Snapshot;
use crate::storage::{Result, SnapshotStore};

/// Redis snapshot store.
///
/// Stores snapshots as simple key-value pairs.
/// Each aggregate root has at most one snapshot.
pub struct RedisSnapshotStore {
    conn: ConnectionManager,
    key_prefix: String,
}

impl RedisSnapshotStore {
    /// Create a new Redis snapshot store.
    ///
    /// # Arguments
    /// * `url` - Redis connection URL (e.g., redis://localhost:6379)
    /// * `key_prefix` - Prefix for all keys (default: "angzarr")
    pub async fn new(url: &str, key_prefix: Option<&str>) -> Result<Self> {
        let client = Client::open(url)?;
        let conn = ConnectionManager::new(client).await?;

        info!(url = %url, "Connected to Redis for snapshots");

        Ok(Self {
            conn,
            key_prefix: key_prefix.unwrap_or("angzarr").to_string(),
        })
    }

    /// Build the snapshot key for a root.
    fn snapshot_key(&self, domain: &str, edition: &str, root: Uuid) -> String {
        format!(
            "{}:{}:{}:{}:snapshot",
            self.key_prefix, domain, edition, root
        )
    }
}

#[async_trait]
impl SnapshotStore for RedisSnapshotStore {
    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let key = self.snapshot_key(domain, edition, root);
        let mut conn = self.conn.clone();

        let bytes: Option<Vec<u8>> = conn.get(&key).await?;

        match bytes {
            Some(b) => {
                let snapshot = Snapshot::decode(b.as_slice())?;
                debug!(domain = %domain, root = %root, "Retrieved snapshot from Redis");
                Ok(Some(snapshot))
            }
            None => Ok(None),
        }
    }

    async fn put(&self, domain: &str, edition: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let key = self.snapshot_key(domain, edition, root);
        let mut conn = self.conn.clone();

        let bytes = snapshot.encode_to_vec();
        let _: () = conn.set(&key, bytes).await?;

        debug!(domain = %domain, root = %root, "Stored snapshot in Redis");
        Ok(())
    }

    async fn delete(&self, domain: &str, edition: &str, root: Uuid) -> Result<()> {
        let key = self.snapshot_key(domain, edition, root);
        let mut conn = self.conn.clone();

        let _: () = conn.del(&key).await?;

        debug!(domain = %domain, root = %root, "Deleted snapshot from Redis");
        Ok(())
    }
}
