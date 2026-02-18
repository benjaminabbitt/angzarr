//! NATS JetStream KV PositionStore implementation.

use async_nats::jetstream::{self, kv::Store, Context};
use async_trait::async_trait;

use crate::storage::{PositionStore, Result, StorageError};

use super::DEFAULT_PREFIX;

/// Bucket name suffix for positions.
const POSITIONS_BUCKET_SUFFIX: &str = "positions";

/// PositionStore backed by NATS JetStream KV.
///
/// Tracks handler checkpoints with keys:
/// `{handler}.{domain}.{root}.{edition}`
pub struct NatsPositionStore {
    kv: Store,
}

impl NatsPositionStore {
    /// Create a new NATS PositionStore.
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
            POSITIONS_BUCKET_SUFFIX
        );

        let kv = Self::ensure_bucket(&jetstream, &bucket_name).await?;
        Ok(Self { kv })
    }

    /// Ensure the KV bucket exists.
    async fn ensure_bucket(
        jetstream: &Context,
        bucket_name: &str,
    ) -> std::result::Result<Store, async_nats::Error> {
        match jetstream.get_key_value(bucket_name).await {
            Ok(store) => Ok(store),
            Err(_) => jetstream
                .create_key_value(jetstream::kv::Config {
                    bucket: bucket_name.to_string(),
                    history: 1, // Only need latest value
                    ..Default::default()
                })
                .await
                .map_err(|e| e.into()),
        }
    }

    /// Build the key for a position entry.
    fn key(handler: &str, domain: &str, edition: &str, root: &[u8]) -> String {
        format!("{}.{}.{}.{}", handler, domain, hex::encode(root), edition)
    }
}

#[async_trait]
impl PositionStore for NatsPositionStore {
    async fn get(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
    ) -> Result<Option<u32>> {
        let key = Self::key(handler, domain, edition, root);

        match self.kv.get(&key).await {
            Ok(Some(entry)) => {
                // Parse u32 from bytes (little-endian)
                // entry is a bytes::Bytes directly
                if entry.len() >= 4 {
                    let bytes: [u8; 4] = entry[..4].try_into().unwrap();
                    Ok(Some(u32::from_le_bytes(bytes)))
                } else {
                    Ok(None)
                }
            }
            Ok(None) => Ok(None),
            Err(e) => {
                // Check if key not found
                let err_str = e.to_string();
                if err_str.contains("key not found") || err_str.contains("not found") {
                    Ok(None)
                } else {
                    Err(StorageError::Nats(format!("Failed to get position: {}", e)))
                }
            }
        }
    }

    async fn put(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
        sequence: u32,
    ) -> Result<()> {
        let key = Self::key(handler, domain, edition, root);
        let value = sequence.to_le_bytes().to_vec();

        self.kv
            .put(&key, value.into())
            .await
            .map_err(|e| StorageError::Nats(format!("Failed to put position: {}", e)))?;

        Ok(())
    }
}
