//! NATS JetStream EventStore implementation.

use async_nats::jetstream::{
    self,
    consumer::pull::Config as ConsumerConfig,
    stream::{Config as StreamConfig, RetentionPolicy, StorageType},
    Context,
};
use async_trait::async_trait;
use futures::StreamExt;
use prost::Message;
use tracing::debug;
use uuid::Uuid;

use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::proto::{event_page, Cover, Edition, EventBook, EventPage, Uuid as ProtoUuid};
use crate::storage::helpers::is_main_timeline;
use crate::storage::{EventStore, Result, StorageError};

use super::DEFAULT_PREFIX;

/// Header name for angzarr sequence number.
const HEADER_SEQUENCE: &str = "Angzarr-Sequence";

/// Header name for correlation ID.
const HEADER_CORRELATION: &str = "Angzarr-Correlation";

/// EventStore backed by NATS JetStream streams.
///
/// Events are stored in per-domain streams with subjects:
/// `{prefix}.events.{domain}.{root}.{edition}`
///
/// Sequence numbers are stored in message headers since JetStream
/// sequences are stream-global, not per-aggregate.
pub struct NatsEventStore {
    jetstream: Context,
    prefix: String,
}

impl NatsEventStore {
    /// Create a new NATS EventStore.
    ///
    /// # Arguments
    /// * `client` - Connected NATS client
    /// * `prefix` - Optional subject prefix (defaults to "angzarr")
    pub async fn new(
        client: async_nats::Client,
        prefix: Option<&str>,
    ) -> std::result::Result<Self, async_nats::Error> {
        let jetstream = jetstream::new(client);
        Ok(Self {
            jetstream,
            prefix: prefix.unwrap_or(DEFAULT_PREFIX).to_string(),
        })
    }

    /// Get the stream name for a domain.
    fn stream_name(&self, domain: &str) -> String {
        format!("{}_{}", self.prefix.to_uppercase(), domain.to_uppercase())
    }

    /// Get the subject for an aggregate.
    fn subject(&self, domain: &str, root: Uuid, edition: &str) -> String {
        format!(
            "{}.events.{}.{}.{}",
            self.prefix,
            domain,
            root.as_hyphenated(),
            edition
        )
    }

    /// Build deduplication message ID for batch (EventBook format).
    /// Format: {domain}.{root}.{edition}.{first_seq}-{last_seq}
    fn msg_id_batch(
        domain: &str,
        root: Uuid,
        edition: &str,
        first_seq: u32,
        last_seq: u32,
    ) -> String {
        format!(
            "{}.{}.{}.{}-{}",
            domain,
            root.as_hyphenated(),
            edition,
            first_seq,
            last_seq
        )
    }

    /// Ensure the stream exists for a domain.
    async fn ensure_stream(&self, domain: &str) -> Result<()> {
        let stream_name = self.stream_name(domain);
        let subjects = format!("{}.events.{}.>", self.prefix, domain);

        match self.jetstream.get_stream(&stream_name).await {
            Ok(_) => Ok(()),
            Err(_) => {
                self.jetstream
                    .create_stream(StreamConfig {
                        name: stream_name,
                        subjects: vec![subjects],
                        retention: RetentionPolicy::Limits,
                        storage: StorageType::File,
                        ..Default::default()
                    })
                    .await
                    .map_err(|e| StorageError::Nats(format!("Failed to create stream: {}", e)))?;
                Ok(())
            }
        }
    }

    /// Get the current max sequence for an aggregate.
    /// Decodes the last EventBook and finds the max sequence from its pages.
    async fn get_max_sequence(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
    ) -> Result<Option<u32>> {
        let stream_name = self.stream_name(domain);
        let subject = self.subject(domain, root, edition);

        let stream = match self.jetstream.get_stream(&stream_name).await {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };

        // Get last message for this specific subject
        match stream.get_last_raw_message_by_subject(&subject).await {
            Ok(msg) => {
                // Decode as EventBook and find max sequence from pages
                if let Ok(book) = EventBook::decode(msg.payload.as_ref()) {
                    let max_seq = book.pages.iter().map(Self::get_sequence).max();
                    return Ok(max_seq);
                }
                Ok(None)
            }
            Err(_) => Ok(None), // No message found or other error
        }
    }

    /// Query events from a specific subject starting at a given sequence.
    /// Reads EventBook messages and extracts pages (same format as EventBus).
    async fn query_events(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        let stream_name = self.stream_name(domain);
        let subject = self.subject(domain, root, edition);

        let stream = match self.jetstream.get_stream(&stream_name).await {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };

        // Create ephemeral consumer for this query
        let consumer_name = format!("query-{}", Uuid::new_v4());
        let consumer = stream
            .create_consumer(ConsumerConfig {
                name: Some(consumer_name),
                filter_subject: subject,
                deliver_policy: jetstream::consumer::DeliverPolicy::All,
                ack_policy: jetstream::consumer::AckPolicy::None,
                ..Default::default()
            })
            .await
            .map_err(|e| StorageError::Nats(format!("Failed to create consumer: {}", e)))?;

        let mut messages = consumer
            .messages()
            .await
            .map_err(|e| StorageError::Nats(format!("Failed to get message stream: {}", e)))?;

        let mut events = Vec::new();

        // Fetch all messages (EventBooks) and extract pages
        while let Ok(Some(msg)) =
            tokio::time::timeout(std::time::Duration::from_millis(100), messages.next()).await
        {
            let msg =
                msg.map_err(|e| StorageError::Nats(format!("Failed to receive message: {}", e)))?;

            // Decode as EventBook (unified format with EventBus)
            let book =
                EventBook::decode(msg.payload.as_ref()).map_err(StorageError::ProtobufDecode)?;

            // Extract pages and filter by sequence
            for page in book.pages {
                let seq = Self::get_sequence(&page);
                if seq >= from {
                    events.push(page);
                }
            }
        }

        // Sort by sequence to ensure order
        events.sort_by_key(|e| Self::get_sequence(e));

        Ok(events)
    }

    /// Extract sequence number from an EventPage.
    fn get_sequence(event: &EventPage) -> u32 {
        match &event.sequence {
            Some(event_page::Sequence::Num(n)) => *n,
            Some(event_page::Sequence::Force(_)) => 0,
            None => 0,
        }
    }

    /// Perform composite read for editions (main timeline up to divergence + edition events).
    async fn composite_read(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        // Get edition events first to find divergence point
        let edition_events = self.query_events(domain, edition, root, 0).await?;

        let divergence = edition_events
            .first()
            .map(Self::get_sequence)
            .unwrap_or(u32::MAX);

        if divergence == u32::MAX {
            // No edition events, just return main timeline
            return self.query_events(domain, DEFAULT_EDITION, root, from).await;
        }

        let mut result = Vec::new();

        // Main timeline events: [from, divergence)
        if from < divergence {
            let main_events = self
                .query_events(domain, DEFAULT_EDITION, root, from)
                .await?;
            for event in main_events {
                if Self::get_sequence(&event) < divergence {
                    result.push(event);
                }
            }
        }

        // Edition events: [max(from, divergence), ∞)
        let edition_from = from.max(divergence);
        for event in edition_events {
            if Self::get_sequence(&event) >= edition_from {
                result.push(event);
            }
        }

        Ok(result)
    }
}

#[async_trait]
impl EventStore for NatsEventStore {
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

        self.ensure_stream(domain).await?;

        // Get current max sequence for validation
        let max_seq = self.get_max_sequence(domain, edition, root).await?;
        let expected_next = max_seq.map(|s| s + 1).unwrap_or(0);
        let first_seq = Self::get_sequence(&events[0]);

        if first_seq != expected_next {
            return Err(StorageError::SequenceConflict {
                expected: expected_next,
                actual: first_seq,
            });
        }

        let subject = self.subject(domain, root, edition);
        let last_seq = events.last().map(Self::get_sequence).unwrap_or(first_seq);

        // Build EventBook (same format as EventBus.publish)
        let book = EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: correlation_id.to_string(),
                edition: Some(Edition {
                    name: edition.to_string(),
                    divergences: vec![],
                }),
            }),
            pages: events,
            snapshot: None,
            next_sequence: last_seq + 1,
        };

        let payload = book.encode_to_vec();

        // Build headers (aligned with EventBus format)
        let mut headers = async_nats::HeaderMap::new();
        headers.insert(HEADER_SEQUENCE, first_seq.to_string().as_str());
        if !correlation_id.is_empty() {
            headers.insert(HEADER_CORRELATION, correlation_id);
        }

        // Deduplication ID: batch format for idempotency with EventBus
        let msg_id = Self::msg_id_batch(domain, root, edition, first_seq, last_seq);
        headers.insert("Nats-Msg-Id", msg_id.as_str());

        self.jetstream
            .publish_with_headers(subject.clone(), headers, payload.into())
            .await
            .map_err(|e| StorageError::Nats(format!("Failed to publish: {}", e)))?
            .await
            .map_err(|e| StorageError::Nats(format!("Publish ack failed: {}", e)))?;

        debug!(
            domain = %domain,
            root = %root,
            first_seq = first_seq,
            last_seq = last_seq,
            msg_id = %msg_id,
            "Published EventBook to NATS"
        );

        Ok(())
    }

    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Vec<EventPage>> {
        self.get_from(domain, edition, root, 0).await
    }

    async fn get_from(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        if is_main_timeline(edition) {
            return self.query_events(domain, DEFAULT_EDITION, root, from).await;
        }

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
        let events = self.get_from(domain, edition, root, from).await?;
        Ok(events
            .into_iter()
            .filter(|e| Self::get_sequence(e) < to)
            .collect())
    }

    async fn list_roots(&self, domain: &str, edition: &str) -> Result<Vec<Uuid>> {
        let stream_name = self.stream_name(domain);
        let subject_prefix = format!("{}.events.{}.", self.prefix, domain);
        let subject_suffix = format!(".{}", edition);

        let stream = match self.jetstream.get_stream(&stream_name).await {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };

        let mut roots = std::collections::HashSet::new();

        // Create consumer to iterate subjects
        let consumer_name = format!("list-roots-{}", Uuid::new_v4());
        let filter = format!("{}.events.{}.*.{}", self.prefix, domain, edition);

        let consumer = match stream
            .create_consumer(ConsumerConfig {
                name: Some(consumer_name),
                filter_subject: filter,
                deliver_policy: jetstream::consumer::DeliverPolicy::All,
                ack_policy: jetstream::consumer::AckPolicy::None,
                ..Default::default()
            })
            .await
        {
            Ok(c) => c,
            Err(_) => return Ok(Vec::new()),
        };

        let mut messages = match consumer.messages().await {
            Ok(m) => m,
            Err(_) => return Ok(Vec::new()),
        };

        // Collect unique roots from message subjects
        while let Ok(Some(msg_result)) =
            tokio::time::timeout(std::time::Duration::from_millis(100), messages.next()).await
        {
            if let Ok(msg) = msg_result {
                // Parse subject: {prefix}.events.{domain}.{root}.{edition}
                if let Some(rest) = msg.subject.as_str().strip_prefix(&subject_prefix) {
                    if let Some(root_str) = rest.strip_suffix(&subject_suffix) {
                        if let Ok(root) = Uuid::parse_str(root_str) {
                            roots.insert(root);
                        }
                    }
                }
            }
        }

        Ok(roots.into_iter().collect())
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        // List all streams with our prefix
        let mut domains = Vec::new();
        let prefix = self.prefix.to_uppercase();

        let mut streams = self.jetstream.streams();
        while let Some(stream) = streams.next().await {
            if let Ok(info) = stream {
                if let Some(domain) = info.config.name.strip_prefix(&format!("{}_", prefix)) {
                    domains.push(domain.to_lowercase());
                }
            }
        }

        Ok(domains)
    }

    async fn get_next_sequence(&self, domain: &str, edition: &str, root: Uuid) -> Result<u32> {
        // For non-main editions, check edition first then fall back to main
        if !is_main_timeline(edition) {
            if let Some(seq) = self.get_max_sequence(domain, edition, root).await? {
                return Ok(seq + 1);
            }
        }

        // Check main timeline
        let target = if is_main_timeline(edition) {
            edition
        } else {
            DEFAULT_EDITION
        };

        let max_seq = self.get_max_sequence(domain, target, root).await?;
        Ok(max_seq.map(|s| s + 1).unwrap_or(0))
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

    async fn get_by_correlation(&self, correlation_id: &str) -> Result<Vec<EventBook>> {
        if correlation_id.is_empty() {
            return Ok(vec![]);
        }

        // This requires scanning all streams - expensive but necessary without a separate index
        // For production, consider a separate correlation index KV bucket
        let domains = self.list_domains().await?;
        let mut books = Vec::new();

        for domain in domains {
            let stream_name = self.stream_name(&domain);
            let stream = match self.jetstream.get_stream(&stream_name).await {
                Ok(s) => s,
                Err(_) => continue,
            };

            // Create consumer to scan all messages
            let consumer_name = format!("correlation-{}", Uuid::new_v4());
            let consumer = match stream
                .create_consumer(ConsumerConfig {
                    name: Some(consumer_name),
                    deliver_policy: jetstream::consumer::DeliverPolicy::All,
                    ack_policy: jetstream::consumer::AckPolicy::None,
                    ..Default::default()
                })
                .await
            {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut messages = match consumer.messages().await {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Group events by (root, edition) - now reading EventBooks
            let mut events_by_root: std::collections::HashMap<(Uuid, String), Vec<EventPage>> =
                std::collections::HashMap::new();

            while let Ok(Some(msg)) =
                tokio::time::timeout(std::time::Duration::from_millis(100), messages.next()).await
            {
                let msg = match msg {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                // Decode as EventBook (unified format)
                let book = match EventBook::decode(msg.payload.as_ref()) {
                    Ok(b) => b,
                    Err(_) => continue,
                };

                // Check correlation from Cover
                let book_correlation = book
                    .cover
                    .as_ref()
                    .map(|c| c.correlation_id.as_str())
                    .unwrap_or("");

                if book_correlation != correlation_id {
                    continue;
                }

                // Extract root and edition from Cover
                let cover = match &book.cover {
                    Some(c) => c,
                    None => continue,
                };

                let root = match &cover.root {
                    Some(r) => match Uuid::from_slice(&r.value) {
                        Ok(u) => u,
                        Err(_) => continue,
                    },
                    None => continue,
                };

                let edition = cover
                    .edition
                    .as_ref()
                    .map(|e| e.name.clone())
                    .unwrap_or_else(|| DEFAULT_EDITION.to_string());

                // Add pages to grouped events
                events_by_root
                    .entry((root, edition))
                    .or_default()
                    .extend(book.pages);
            }

            // Build EventBooks from grouped events
            for ((root, edition), mut pages) in events_by_root {
                pages.sort_by_key(Self::get_sequence);
                let next_seq = pages.last().map(Self::get_sequence).unwrap_or(0) + 1;

                books.push(EventBook {
                    cover: Some(Cover {
                        domain: domain.clone(),
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
                    next_sequence: next_seq,
                });
            }
        }

        Ok(books)
    }

    async fn delete_edition_events(&self, domain: &str, edition: &str) -> Result<u32> {
        // NATS JetStream doesn't support targeted deletion by subject filter easily
        // We need to purge messages matching our subject pattern
        let stream_name = self.stream_name(domain);
        let subject_pattern = format!("{}.events.{}.*.{}", self.prefix, domain, edition);

        let mut stream = match self.jetstream.get_stream(&stream_name).await {
            Ok(s) => s,
            Err(_) => return Ok(0),
        };

        // Get count before purge
        let count_before = stream
            .info()
            .await
            .map_err(|e| StorageError::Nats(format!("Failed to get stream info: {}", e)))?
            .state
            .messages;

        // Purge messages matching subject
        stream
            .purge()
            .filter(&subject_pattern)
            .await
            .map_err(|e| StorageError::Nats(format!("Failed to purge: {}", e)))?;

        let count_after = stream
            .info()
            .await
            .map_err(|e| StorageError::Nats(format!("Failed to get stream info: {}", e)))?
            .state
            .messages;

        let deleted = (count_before - count_after) as u32;

        debug!(
            domain = %domain,
            edition = %edition,
            deleted = deleted,
            "Purged edition events from NATS"
        );

        Ok(deleted)
    }
}
