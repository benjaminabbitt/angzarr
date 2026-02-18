//! NATS JetStream KV SnapshotStore implementation.

use async_nats::jetstream::{self, kv::Store, Context};
use async_trait::async_trait;
use prost::Message;
use uuid::Uuid;

use crate::proto::Snapshot;
use crate::storage::{Result, SnapshotStore, StorageError};

use super::DEFAULT_PREFIX;

/// Bucket name suffix for snapshots.
const SNAPSHOTS_BUCKET_SUFFIX: &str = "snapshots";

/// Maximum history depth for snapshot KV bucket.
/// Enables `get_at_seq()` to find historical snapshots.
const SNAPSHOT_HISTORY: i64 = 64;

/// SnapshotStore backed by NATS JetStream KV with history.
///
/// Stores snapshots with keys:
/// `{domain}.{root}.{edition}`
///
/// History is enabled to support `get_at_seq()` for
/// finding snapshots at specific sequence numbers.
pub struct NatsSnapshotStore {
    kv: Store,
}

impl NatsSnapshotStore {
    /// Create a new NATS SnapshotStore.
    ///
    /// # Arguments
    /// * `client` - Connected NATS client
    /// * `prefix` - Optional bucket prefix (defaults to "angzarr")
    pub async fn new(
        client: async_nats::Client,
        prefix: Option<&str>,
    ) -> std::result::Result<Self, async_nats::Error> {
        let jetstream = jetstream::new(client);
        let bucket_name = format!(
            "{}-{}",
            prefix.unwrap_or(DEFAULT_PREFIX),
            SNAPSHOTS_BUCKET_SUFFIX
        );

        let kv = Self::ensure_bucket(&jetstream, &bucket_name).await?;
        Ok(Self { kv })
    }

    /// Ensure the KV bucket exists with history enabled.
    async fn ensure_bucket(
        jetstream: &Context,
        bucket_name: &str,
    ) -> std::result::Result<Store, async_nats::Error> {
        match jetstream.get_key_value(bucket_name).await {
            Ok(store) => Ok(store),
            Err(_) => jetstream
                .create_key_value(jetstream::kv::Config {
                    bucket: bucket_name.to_string(),
                    history: SNAPSHOT_HISTORY,
                    max_value_size: 10 * 1024 * 1024, // 10MB max snapshot
                    ..Default::default()
                })
                .await
                .map_err(|e| e.into()),
        }
    }

    /// Build the key for a snapshot entry.
    fn key(domain: &str, edition: &str, root: Uuid) -> String {
        format!("{}.{}.{}", domain, root.as_hyphenated(), edition)
    }
}

#[async_trait]
impl SnapshotStore for NatsSnapshotStore {
    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let key = Self::key(domain, edition, root);

        match self.kv.get(&key).await {
            Ok(Some(entry)) => {
                // entry is a bytes::Bytes directly
                let snapshot =
                    Snapshot::decode(entry.as_ref()).map_err(StorageError::ProtobufDecode)?;
                Ok(Some(snapshot))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("key not found") || err_str.contains("not found") {
                    Ok(None)
                } else {
                    Err(StorageError::Nats(format!("Failed to get snapshot: {}", e)))
                }
            }
        }
    }

    async fn get_at_seq(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        seq: u32,
    ) -> Result<Option<Snapshot>> {
        let key = Self::key(domain, edition, root);

        // Get history for this key and find the snapshot with matching or lower sequence
        let history = match self.kv.history(&key).await {
            Ok(h) => h,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("key not found") || err_str.contains("not found") {
                    return Ok(None);
                }
                return Err(StorageError::Nats(format!(
                    "Failed to get snapshot history: {}",
                    e
                )));
            }
        };

        // Collect history entries
        use futures::StreamExt;
        let entries: Vec<_> = history.collect().await;

        // Find the most recent snapshot with sequence <= requested seq
        let mut best_match: Option<Snapshot> = None;
        let mut best_seq: u32 = 0;

        for entry_result in entries {
            if let Ok(entry) = entry_result {
                if let Ok(snapshot) = Snapshot::decode(entry.value.as_ref()) {
                    if snapshot.sequence <= seq && snapshot.sequence >= best_seq {
                        best_seq = snapshot.sequence;
                        best_match = Some(snapshot);
                    }
                }
            }
        }

        Ok(best_match)
    }

    async fn put(&self, domain: &str, edition: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let key = Self::key(domain, edition, root);
        let value = snapshot.encode_to_vec();

        self.kv
            .put(&key, value.into())
            .await
            .map_err(|e| StorageError::Nats(format!("Failed to put snapshot: {}", e)))?;

        Ok(())
    }

    async fn delete(&self, domain: &str, edition: &str, root: Uuid) -> Result<()> {
        let key = Self::key(domain, edition, root);

        // Delete the key (purges all history)
        match self.kv.purge(&key).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("key not found") || err_str.contains("not found") {
                    Ok(()) // Already deleted, that's fine
                } else {
                    Err(StorageError::Nats(format!(
                        "Failed to delete snapshot: {}",
                        e
                    )))
                }
            }
        }
    }
}
