//! Redis storage implementations.
//!
//! Stores events and snapshots in Redis.
//! Key structure:
//! - `{prefix}:{domain}:{root}:events` - Sorted set of events by sequence
//! - `{prefix}:{domain}:{root}:snapshot` - Latest snapshot (binary)
//! - `{prefix}:{domain}:roots` - Set of all root IDs in domain
//! - `{prefix}:domains` - Set of all domains

use async_trait::async_trait;
use prost::Message;
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use tracing::{debug, info};
use uuid::Uuid;

use super::{EventStore, Result, SnapshotStore, StorageError};
use crate::proto::{EventPage, Snapshot};

/// Redis event store.
///
/// Uses sorted sets to store events ordered by sequence number.
/// Provides efficient range queries and append-only semantics.
pub struct RedisEventStore {
    conn: ConnectionManager,
    key_prefix: String,
}

impl RedisEventStore {
    /// Create a new Redis event store.
    ///
    /// # Arguments
    /// * `url` - Redis connection URL (e.g., redis://localhost:6379)
    /// * `key_prefix` - Prefix for all keys (default: "angzarr")
    pub async fn new(url: &str, key_prefix: Option<&str>) -> Result<Self> {
        let client = Client::open(url)?;
        let conn = ConnectionManager::new(client).await?;

        info!(url = %url, "Connected to Redis");

        Ok(Self {
            conn,
            key_prefix: key_prefix.unwrap_or("angzarr").to_string(),
        })
    }

    /// Build the events key for a root.
    fn events_key(&self, domain: &str, root: Uuid) -> String {
        format!("{}:{}:{}:events", self.key_prefix, domain, root)
    }

    /// Build the roots set key for a domain.
    fn roots_key(&self, domain: &str) -> String {
        format!("{}:{}:roots", self.key_prefix, domain)
    }

    /// Build the domains set key.
    fn domains_key(&self) -> String {
        format!("{}:domains", self.key_prefix)
    }

    /// Serialize an event page to bytes.
    fn serialize_event(event: &EventPage) -> Result<Vec<u8>> {
        Ok(event.encode_to_vec())
    }

    /// Deserialize bytes to an event page.
    fn deserialize_event(bytes: &[u8]) -> Result<EventPage> {
        EventPage::decode(bytes).map_err(StorageError::ProtobufDecode)
    }

    /// Get sequence number from event page.
    fn get_sequence(event: &EventPage) -> u32 {
        match &event.sequence {
            Some(crate::proto::event_page::Sequence::Num(n)) => *n,
            Some(crate::proto::event_page::Sequence::Force(_)) => 0,
            None => 0,
        }
    }
}

#[async_trait]
impl EventStore for RedisEventStore {
    async fn add(
        &self,
        domain: &str,
        root: Uuid,
        events: Vec<EventPage>,
        _correlation_id: &str,
    ) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let events_key = self.events_key(domain, root);
        let roots_key = self.roots_key(domain);
        let mut conn = self.conn.clone();

        // Get current max sequence
        let max_seq: Option<f64> = conn
            .zrevrange_withscores::<_, Vec<(Vec<u8>, f64)>>(&events_key, 0, 0)
            .await?
            .first()
            .map(|(_, score)| *score);

        let expected_next = max_seq.map(|s| s as u32 + 1).unwrap_or(0);
        let first_seq = Self::get_sequence(&events[0]);

        // Validate sequence continuity
        if first_seq != expected_next {
            return Err(StorageError::SequenceConflict {
                expected: expected_next,
                actual: first_seq,
            });
        }

        // Prepare events for insertion
        let mut items: Vec<(f64, Vec<u8>)> = Vec::with_capacity(events.len());
        for event in &events {
            let seq = Self::get_sequence(event);
            let bytes = Self::serialize_event(event)?;
            items.push((seq as f64, bytes));
        }

        // Add events to sorted set
        let _: () = conn.zadd_multiple(&events_key, &items).await?;

        // Track root in domain set
        let _: () = conn.sadd(&roots_key, root.to_string()).await?;

        // Track domain in domains set
        let domains_key = self.domains_key();
        let _: () = conn.sadd(&domains_key, domain).await?;

        debug!(
            domain = %domain,
            root = %root,
            count = events.len(),
            "Stored events in Redis"
        );

        Ok(())
    }

    async fn get(&self, domain: &str, root: Uuid) -> Result<Vec<EventPage>> {
        let events_key = self.events_key(domain, root);
        let mut conn = self.conn.clone();

        let bytes_list: Vec<Vec<u8>> = conn.zrange(&events_key, 0, -1).await?;

        let events: Result<Vec<EventPage>> = bytes_list
            .iter()
            .map(|b| Self::deserialize_event(b))
            .collect();

        events
    }

    async fn get_from(&self, domain: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>> {
        let events_key = self.events_key(domain, root);
        let mut conn = self.conn.clone();

        let bytes_list: Vec<Vec<u8>> = conn.zrangebyscore(&events_key, from as f64, "+inf").await?;

        let events: Result<Vec<EventPage>> = bytes_list
            .iter()
            .map(|b| Self::deserialize_event(b))
            .collect();

        events
    }

    async fn get_from_to(
        &self,
        domain: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>> {
        let events_key = self.events_key(domain, root);
        let mut conn = self.conn.clone();

        // Redis ZRANGEBYSCORE is inclusive, but our interface uses exclusive end
        let bytes_list: Vec<Vec<u8>> = conn
            .zrangebyscore(&events_key, from as f64, (to - 1) as f64)
            .await?;

        let events: Result<Vec<EventPage>> = bytes_list
            .iter()
            .map(|b| Self::deserialize_event(b))
            .collect();

        events
    }

    async fn get_until_timestamp(
        &self,
        domain: &str,
        root: Uuid,
        until: &str,
    ) -> Result<Vec<EventPage>> {
        let until_dt = chrono::DateTime::parse_from_rfc3339(until)
            .map_err(|e| StorageError::InvalidTimestampFormat(e.to_string()))?;

        let all_events = self.get(domain, root).await?;

        Ok(all_events
            .into_iter()
            .filter(|e| {
                if let Some(ref ts) = e.created_at {
                    if let Some(dt) = chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
                    {
                        return dt <= until_dt;
                    }
                }
                false
            })
            .collect())
    }

    async fn list_roots(&self, domain: &str) -> Result<Vec<Uuid>> {
        let roots_key = self.roots_key(domain);
        let mut conn = self.conn.clone();

        let root_strings: Vec<String> = conn.smembers(&roots_key).await?;

        let roots: Result<Vec<Uuid>> = root_strings
            .iter()
            .map(|s| Uuid::parse_str(s).map_err(StorageError::InvalidUuid))
            .collect();

        roots
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        let domains_key = self.domains_key();
        let mut conn = self.conn.clone();

        let domains: Vec<String> = conn.smembers(&domains_key).await?;
        Ok(domains)
    }

    async fn get_next_sequence(&self, domain: &str, root: Uuid) -> Result<u32> {
        let events_key = self.events_key(domain, root);
        let mut conn = self.conn.clone();

        let max_seq: Option<f64> = conn
            .zrevrange_withscores::<_, Vec<(Vec<u8>, f64)>>(&events_key, 0, 0)
            .await?
            .first()
            .map(|(_, score)| *score);

        Ok(max_seq.map(|s| s as u32 + 1).unwrap_or(0))
    }

    async fn get_by_correlation(
        &self,
        _correlation_id: &str,
    ) -> Result<Vec<crate::proto::EventBook>> {
        // Not implemented for Redis - correlation_id not indexed
        Ok(vec![])
    }
}

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
    fn snapshot_key(&self, domain: &str, root: Uuid) -> String {
        format!("{}:{}:{}:snapshot", self.key_prefix, domain, root)
    }
}

#[async_trait]
impl SnapshotStore for RedisSnapshotStore {
    async fn get(&self, domain: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let key = self.snapshot_key(domain, root);
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

    async fn put(&self, domain: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let key = self.snapshot_key(domain, root);
        let mut conn = self.conn.clone();

        let bytes = snapshot.encode_to_vec();
        let _: () = conn.set(&key, bytes).await?;

        debug!(domain = %domain, root = %root, "Stored snapshot in Redis");
        Ok(())
    }

    async fn delete(&self, domain: &str, root: Uuid) -> Result<()> {
        let key = self.snapshot_key(domain, root);
        let mut conn = self.conn.clone();

        let _: () = conn.del(&key).await?;

        debug!(domain = %domain, root = %root, "Deleted snapshot from Redis");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests require Redis running
    // Run with: cargo test --features redis -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_redis_event_store() {
        let store = RedisEventStore::new("redis://localhost:6379", Some("test"))
            .await
            .expect("Failed to connect to Redis");

        let domain = "test-domain";
        let root = Uuid::new_v4();

        // Create test events
        let events = vec![
            EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(0)),
                event: None,
                created_at: None,
            },
            EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(1)),
                event: None,
                created_at: None,
            },
        ];

        // Add events
        store
            .add(domain, root, events.clone(), "")
            .await
            .expect("Failed to add events");

        // Retrieve events
        let retrieved = store.get(domain, root).await.expect("Failed to get events");
        assert_eq!(retrieved.len(), 2);

        // Check next sequence
        let next_seq = store
            .get_next_sequence(domain, root)
            .await
            .expect("Failed to get next sequence");
        assert_eq!(next_seq, 2);

        // List roots
        let roots = store
            .list_roots(domain)
            .await
            .expect("Failed to list roots");
        assert!(roots.contains(&root));
    }
}
