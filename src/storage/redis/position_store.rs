//! Redis PositionStore implementation.
//!
//! Stores handler checkpoint positions using simple key-value pairs.
//! Each (handler, domain, edition, root) tuple has a single sequence number.

use async_trait::async_trait;
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use tracing::{debug, info};

use crate::storage::{PositionStore, Result};

/// Redis implementation of PositionStore.
///
/// Stores positions as simple key-value pairs.
/// Key format: `{prefix}:position:{handler}:{domain}:{edition}:{root_hex}`
pub struct RedisPositionStore {
    conn: ConnectionManager,
    key_prefix: String,
}

impl RedisPositionStore {
    /// Create a new Redis position store.
    ///
    /// # Arguments
    /// * `url` - Redis connection URL (e.g., redis://localhost:6379)
    /// * `key_prefix` - Prefix for all keys (default: "angzarr")
    pub async fn new(url: &str, key_prefix: Option<&str>) -> Result<Self> {
        let client = Client::open(url)?;
        let conn = ConnectionManager::new(client).await?;

        info!(url = %url, "Connected to Redis for positions");

        Ok(Self {
            conn,
            key_prefix: key_prefix.unwrap_or("angzarr").to_string(),
        })
    }

    /// Build the position key for a handler/domain/edition/root.
    fn position_key(&self, handler: &str, domain: &str, edition: &str, root: &[u8]) -> String {
        // Encode root bytes as hex for key safety
        let root_hex = hex::encode(root);
        format!(
            "{}:position:{}:{}:{}:{}",
            self.key_prefix, handler, domain, edition, root_hex
        )
    }
}

#[async_trait]
impl PositionStore for RedisPositionStore {
    async fn get(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
    ) -> Result<Option<u32>> {
        let key = self.position_key(handler, domain, edition, root);
        let mut conn = self.conn.clone();

        let value: Option<u32> = conn.get(&key).await?;

        if value.is_some() {
            debug!(
                handler = %handler,
                domain = %domain,
                edition = %edition,
                "Retrieved position from Redis"
            );
        }

        Ok(value)
    }

    async fn put(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
        sequence: u32,
    ) -> Result<()> {
        let key = self.position_key(handler, domain, edition, root);
        let mut conn = self.conn.clone();

        let _: () = conn.set(&key, sequence).await?;

        debug!(
            handler = %handler,
            domain = %domain,
            edition = %edition,
            sequence = sequence,
            "Stored position in Redis"
        );

        Ok(())
    }
}
