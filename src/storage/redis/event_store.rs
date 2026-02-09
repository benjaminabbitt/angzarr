//! Redis EventStore implementation.
//!
//! Implements composite reads for editions: query edition events first to derive
//! the implicit divergence point, then query main timeline up to that point,
//! then merge the results.

use async_trait::async_trait;
use prost::Message;
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use tracing::{debug, info};
use uuid::Uuid;

use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::proto::EventPage;
use crate::storage::{EventStore, Result, StorageError};

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
    fn events_key(&self, domain: &str, edition: &str, root: Uuid) -> String {
        format!("{}:{}:{}:{}:events", self.key_prefix, domain, edition, root)
    }

    /// Build the roots set key for a domain.
    fn roots_key(&self, domain: &str, edition: &str) -> String {
        format!("{}:{}:{}:roots", self.key_prefix, domain, edition)
    }

    /// Build the domains set key.
    fn domains_key(&self) -> String {
        format!("{}:domains", self.key_prefix)
    }

    /// Build the correlation index key.
    fn correlation_key(&self, correlation_id: &str) -> String {
        format!("{}:correlation:{}", self.key_prefix, correlation_id)
    }

    /// Build an event reference for correlation index.
    /// Format: domain:edition:root:sequence
    fn event_ref(domain: &str, edition: &str, root: Uuid, sequence: u32) -> String {
        format!("{}:{}:{}:{}", domain, edition, root, sequence)
    }

    /// Parse an event reference from correlation index.
    fn parse_event_ref(event_ref: &str) -> Option<(String, String, Uuid, u32)> {
        let parts: Vec<&str> = event_ref.splitn(4, ':').collect();
        if parts.len() == 4 {
            let domain = parts[0].to_string();
            let edition = parts[1].to_string();
            let root = Uuid::parse_str(parts[2]).ok()?;
            let sequence: u32 = parts[3].parse().ok()?;
            Some((domain, edition, root, sequence))
        } else {
            None
        }
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

    /// Check if edition is the main timeline.
    fn is_main_timeline(edition: &str) -> bool {
        edition.is_empty() || edition == DEFAULT_EDITION
    }

    /// Query events for a specific edition (internal helper).
    async fn query_edition_events(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        let events_key = self.events_key(domain, edition, root);
        let mut conn = self.conn.clone();

        let bytes_list: Vec<Vec<u8>> = conn.zrangebyscore(&events_key, from as f64, "+inf").await?;

        let events: Result<Vec<EventPage>> = bytes_list
            .iter()
            .map(|b| Self::deserialize_event(b))
            .collect();

        events
    }

    /// Get the minimum sequence number from edition events (implicit divergence point).
    async fn get_edition_min_sequence(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
    ) -> Result<Option<u32>> {
        let events_key = self.events_key(domain, edition, root);
        let mut conn = self.conn.clone();

        // Get the first element (minimum score) from the sorted set
        let result: Vec<(Vec<u8>, f64)> = conn.zrange_withscores(&events_key, 0, 0).await?;

        match result.first() {
            Some((_, score)) => Ok(Some(*score as u32)),
            None => Ok(None),
        }
    }

    /// Query main timeline events in range [from, until).
    async fn query_main_events_range(
        &self,
        domain: &str,
        root: Uuid,
        from: u32,
        until_seq: u32,
    ) -> Result<Vec<EventPage>> {
        if from >= until_seq {
            return Ok(Vec::new());
        }

        let events_key = self.events_key(domain, DEFAULT_EDITION, root);
        let mut conn = self.conn.clone();

        // Redis ZRANGEBYSCORE is inclusive on both ends, so use until_seq - 1
        let bytes_list: Vec<Vec<u8>> = conn
            .zrangebyscore(&events_key, from as f64, (until_seq - 1) as f64)
            .await?;

        let events: Result<Vec<EventPage>> = bytes_list
            .iter()
            .map(|b| Self::deserialize_event(b))
            .collect();

        events
    }

    /// Perform a composite read for an edition.
    ///
    /// Optimized to avoid full edition scan:
    /// 1. Query only the min sequence (divergence point) - O(log n)
    /// 2. Fetch only needed events based on `from` parameter
    async fn composite_read(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        // Get divergence point without fetching all edition events
        let divergence = match self.get_edition_min_sequence(domain, edition, root).await? {
            Some(d) => d,
            None => {
                // No edition events - return main timeline only
                return self
                    .query_edition_events(domain, DEFAULT_EDITION, root, from)
                    .await;
            }
        };

        // Now fetch only the events we need:
        // - Main timeline: [from, divergence) if from < divergence
        // - Edition: [max(from, divergence), ∞)

        let mut result = Vec::new();

        // Main timeline events: only if from < divergence
        if from < divergence {
            let main_events = self
                .query_main_events_range(domain, root, from, divergence)
                .await?;
            result.extend(main_events);
        }

        // Edition events: from max(from, divergence) onwards
        let edition_from = from.max(divergence);
        let edition_events = self
            .query_edition_events(domain, edition, root, edition_from)
            .await?;
        result.extend(edition_events);

        Ok(result)
    }
}

#[async_trait]
impl EventStore for RedisEventStore {
    async fn add(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        events: Vec<EventPage>,
        correlation_id: &str,
    ) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let events_key = self.events_key(domain, edition, root);
        let roots_key = self.roots_key(domain, edition);
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

        // Prepare events for insertion and collect event refs for correlation index
        let mut items: Vec<(f64, Vec<u8>)> = Vec::with_capacity(events.len());
        let mut event_refs: Vec<String> = Vec::new();

        for event in &events {
            let seq = Self::get_sequence(event);
            let bytes = Self::serialize_event(event)?;
            items.push((seq as f64, bytes));

            // Build event reference for correlation index
            if !correlation_id.is_empty() {
                event_refs.push(Self::event_ref(domain, edition, root, seq));
            }
        }

        // Add events to sorted set
        let _: () = conn.zadd_multiple(&events_key, &items).await?;

        // Track root in domain set
        let _: () = conn.sadd(&roots_key, root.to_string()).await?;

        // Track domain in domains set
        let domains_key = self.domains_key();
        let _: () = conn.sadd(&domains_key, domain).await?;

        // Add to correlation index if correlation_id is provided
        if !correlation_id.is_empty() && !event_refs.is_empty() {
            let correlation_key = self.correlation_key(correlation_id);
            for event_ref in event_refs {
                let _: () = conn.sadd(&correlation_key, event_ref).await?;
            }
        }

        debug!(
            domain = %domain,
            root = %root,
            count = events.len(),
            correlation_id = %correlation_id,
            "Stored events in Redis"
        );

        Ok(())
    }

    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Vec<EventPage>> {
        let events_key = self.events_key(domain, edition, root);
        let mut conn = self.conn.clone();

        let bytes_list: Vec<Vec<u8>> = conn.zrange(&events_key, 0, -1).await?;

        let events: Result<Vec<EventPage>> = bytes_list
            .iter()
            .map(|b| Self::deserialize_event(b))
            .collect();

        events
    }

    async fn get_from(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        // Main timeline: simple query
        if Self::is_main_timeline(edition) {
            return self
                .query_edition_events(domain, DEFAULT_EDITION, root, from)
                .await;
        }

        // Named edition: composite read (main timeline up to divergence + edition events)
        self.composite_read(domain, edition, root, from).await
    }

    async fn get_from_to(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>> {
        let events_key = self.events_key(domain, edition, root);
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
        edition: &str,
        root: Uuid,
        until: &str,
    ) -> Result<Vec<EventPage>> {
        let until_dt = chrono::DateTime::parse_from_rfc3339(until)
            .map_err(|e| StorageError::InvalidTimestampFormat(e.to_string()))?;

        let all_events = self.get(domain, edition, root).await?;

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

    async fn list_roots(&self, domain: &str, edition: &str) -> Result<Vec<Uuid>> {
        let roots_key = self.roots_key(domain, edition);
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

    async fn get_next_sequence(&self, domain: &str, edition: &str, root: Uuid) -> Result<u32> {
        let mut conn = self.conn.clone();

        // For non-default editions with implicit divergence, we need composite logic:
        // If the edition has no events yet, use the main timeline's max sequence
        if !Self::is_main_timeline(edition) {
            let edition_key = self.events_key(domain, edition, root);
            let max_seq: Option<f64> = conn
                .zrevrange_withscores::<_, Vec<(Vec<u8>, f64)>>(&edition_key, 0, 0)
                .await?
                .first()
                .map(|(_, score)| *score);

            if let Some(seq) = max_seq {
                // Edition has events, use edition's max sequence
                return Ok(seq as u32 + 1);
            }

            // No edition events - fall through to check main timeline
        }

        // Query the target edition (or main timeline for fallback)
        let target_edition = if Self::is_main_timeline(edition) {
            edition
        } else {
            DEFAULT_EDITION
        };

        let events_key = self.events_key(domain, target_edition, root);
        let max_seq: Option<f64> = conn
            .zrevrange_withscores::<_, Vec<(Vec<u8>, f64)>>(&events_key, 0, 0)
            .await?
            .first()
            .map(|(_, score)| *score);

        Ok(max_seq.map(|s| s as u32 + 1).unwrap_or(0))
    }

    async fn get_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Vec<crate::proto::EventBook>> {
        use crate::proto::{Cover, Edition, EventBook, Uuid as ProtoUuid};
        use std::collections::HashMap;

        if correlation_id.is_empty() {
            return Ok(vec![]);
        }

        let correlation_key = self.correlation_key(correlation_id);
        let mut conn = self.conn.clone();

        // Get all event references for this correlation ID
        let event_refs: Vec<String> = conn.smembers(&correlation_key).await?;

        if event_refs.is_empty() {
            return Ok(vec![]);
        }

        // Group event references by (domain, edition, root)
        let mut refs_by_root: HashMap<(String, String, Uuid), Vec<u32>> = HashMap::new();

        for event_ref in &event_refs {
            if let Some((domain, edition, root, sequence)) = Self::parse_event_ref(event_ref) {
                refs_by_root
                    .entry((domain, edition, root))
                    .or_default()
                    .push(sequence);
            }
        }

        // Fetch events for each unique (domain, edition, root) and filter by sequences
        let mut books = Vec::new();

        for ((domain, edition, root), sequences) in refs_by_root {
            let events_key = self.events_key(&domain, &edition, root);

            // Fetch all events for this root (we need to filter by sequence)
            let bytes_list: Vec<(Vec<u8>, f64)> =
                conn.zrange_withscores(&events_key, 0, -1).await?;

            let mut pages = Vec::new();
            for (bytes, score) in bytes_list {
                let seq = score as u32;
                if sequences.contains(&seq) {
                    let event = Self::deserialize_event(&bytes)?;
                    pages.push(event);
                }
            }

            // Sort pages by sequence
            pages.sort_by_key(Self::get_sequence);

            if !pages.is_empty() {
                books.push(EventBook {
                    cover: Some(Cover {
                        domain,
                        root: Some(ProtoUuid {
                            value: root.as_bytes().to_vec(),
                        }),
                        correlation_id: correlation_id.to_string(),
                        edition: Some(Edition {
                            name: edition,
                            divergences: vec![],
                        }),
                    }),
                    pages,
                    snapshot: None,
                });
            }
        }

        Ok(books)
    }

    async fn delete_edition_events(&self, domain: &str, edition: &str) -> Result<u32> {
        let mut conn = self.conn.clone();
        let mut deleted_count = 0u32;

        // Pattern to find all event keys for this domain/edition
        // Format: {prefix}:{domain}:{edition}:*:events
        let pattern = format!("{}:{}:{}:*:events", self.key_prefix, domain, edition);

        // Use SCAN to find matching keys (non-blocking iteration)
        let mut cursor = 0u64;
        let mut keys_to_delete: Vec<String> = Vec::new();

        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;

            keys_to_delete.extend(keys);
            cursor = next_cursor;

            if cursor == 0 {
                break;
            }
        }

        // Delete found event keys and count events
        for key in &keys_to_delete {
            // Count events in this key before deleting
            let count: u32 = conn.zcard(key).await.unwrap_or(0) as u32;
            deleted_count += count;

            // Delete the sorted set
            let _: () = conn.del(key).await?;

            // Extract root from key to remove from roots set
            // Key format: {prefix}:{domain}:{edition}:{root}:events
            if let Some(root_str) = key
                .strip_prefix(&format!("{}:{}:{}:", self.key_prefix, domain, edition))
                .and_then(|s| s.strip_suffix(":events"))
            {
                let roots_key = self.roots_key(domain, edition);
                let _: () = conn.srem(&roots_key, root_str).await?;
            }
        }

        debug!(
            domain = %domain,
            edition = %edition,
            keys_deleted = keys_to_delete.len(),
            events_deleted = deleted_count,
            "Deleted edition events from Redis"
        );

        Ok(deleted_count)
    }
}
