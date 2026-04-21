//! DynamoDB EventStore implementation.
//!
//! Table schema:
//! - PK: `{domain}#{edition}#{root}` (String)
//! - SK: sequence number (Number)
//! - event: serialized EventPage (Binary)
//! - created_at: ISO 8601 timestamp (String)
//! - correlation_id: for cross-domain queries (String)
//! - committed: cascade commit status (Boolean)
//! - cascade_id: cascade identifier (String, sparse)
//!
//! GSI `correlation-index`:
//! - PK: correlation_id
//! - SK: `{domain}#{edition}#{root}#{seq}`
//!
//! GSI `cascade-index`:
//! - PK: cascade_id
//! - SK: pk (main table partition key)

use std::collections::HashMap;

use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use prost::Message;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::proto::{Cover, Edition, EventBook, EventPage, Uuid as ProtoUuid};
use crate::proto_ext::EventPageExt;
use crate::storage::helpers::is_main_timeline;
use crate::storage::{
    AddOutcome, CascadeParticipant, EventStore, Result, SourceInfo, StorageError,
};

/// DynamoDB implementation of EventStore.
pub struct DynamoEventStore {
    client: Client,
    table_name: String,
}

impl DynamoEventStore {
    /// Create a new DynamoDB event store.
    pub async fn new(table_name: impl Into<String>, endpoint_url: Option<&str>) -> Result<Self> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;

        let client = if let Some(endpoint) = endpoint_url {
            let dynamo_config = aws_sdk_dynamodb::config::Builder::from(&config)
                .endpoint_url(endpoint)
                .build();
            Client::from_conf(dynamo_config)
        } else {
            Client::new(&config)
        };

        let table_name = table_name.into();
        info!(table = %table_name, "Connected to DynamoDB for events");

        Ok(Self { client, table_name })
    }

    /// Build the partition key for events.
    fn pk(domain: &str, edition: &str, root: Uuid) -> String {
        format!("{}#{}#{}", domain, edition, root)
    }

    /// Parse partition key into (domain, edition, root).
    fn parse_pk(pk: &str) -> Option<(String, String, Uuid)> {
        let parts: Vec<&str> = pk.splitn(3, '#').collect();
        if parts.len() == 3 {
            let root = Uuid::parse_str(parts[2]).ok()?;
            Some((parts[0].to_string(), parts[1].to_string(), root))
        } else {
            None
        }
    }

    /// Get sequence from EventPage.
    fn get_sequence(event: &EventPage) -> u32 {
        event.sequence_num()
    }

    /// Query events for a specific edition.
    async fn query_edition_events(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        let pk = Self::pk(domain, edition, root);

        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("pk = :pk AND seq >= :from")
            .expression_attribute_values(":pk", AttributeValue::S(pk))
            .expression_attribute_values(":from", AttributeValue::N(from.to_string()))
            .send()
            .await
            .map_err(|e| StorageError::NotImplemented(format!("DynamoDB query failed: {}", e)))?;

        let mut events = Vec::new();
        if let Some(items) = result.items {
            for item in items {
                if let Some(AttributeValue::B(blob)) = item.get("event") {
                    let event =
                        EventPage::decode(blob.as_ref()).map_err(StorageError::ProtobufDecode)?;
                    events.push(event);
                }
            }
        }

        Ok(events)
    }

    /// Get minimum sequence from edition events (divergence point).
    async fn get_edition_min_sequence(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
    ) -> Result<Option<u32>> {
        let pk = Self::pk(domain, edition, root);

        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("pk = :pk")
            .expression_attribute_values(":pk", AttributeValue::S(pk))
            .limit(1)
            .send()
            .await
            .map_err(|e| StorageError::NotImplemented(format!("DynamoDB query failed: {}", e)))?;

        if let Some(items) = result.items {
            if let Some(item) = items.first() {
                if let Some(AttributeValue::N(seq_str)) = item.get("seq") {
                    return Ok(seq_str.parse().ok());
                }
            }
        }

        Ok(None)
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

        let pk = Self::pk(domain, DEFAULT_EDITION, root);

        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("pk = :pk AND seq BETWEEN :from AND :to")
            .expression_attribute_values(":pk", AttributeValue::S(pk))
            .expression_attribute_values(":from", AttributeValue::N(from.to_string()))
            .expression_attribute_values(":to", AttributeValue::N((until_seq - 1).to_string()))
            .send()
            .await
            .map_err(|e| StorageError::NotImplemented(format!("DynamoDB query failed: {}", e)))?;

        let mut events = Vec::new();
        if let Some(items) = result.items {
            for item in items {
                if let Some(AttributeValue::B(blob)) = item.get("event") {
                    let event =
                        EventPage::decode(blob.as_ref()).map_err(StorageError::ProtobufDecode)?;
                    events.push(event);
                }
            }
        }

        Ok(events)
    }

    /// Composite read for editions (main timeline up to divergence + edition events).
    async fn composite_read(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        let divergence = match self.get_edition_min_sequence(domain, edition, root).await? {
            Some(d) => d,
            None => {
                return self
                    .query_edition_events(domain, DEFAULT_EDITION, root, from)
                    .await;
            }
        };

        let mut result = Vec::new();

        if from < divergence {
            let main_events = self
                .query_main_events_range(domain, root, from, divergence)
                .await?;
            result.extend(main_events);
        }

        let edition_from = from.max(divergence);
        let edition_events = self
            .query_edition_events(domain, edition, root, edition_from)
            .await?;
        result.extend(edition_events);

        Ok(result)
    }
}

#[async_trait]
impl EventStore for DynamoEventStore {
    async fn add(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        events: Vec<EventPage>,
        correlation_id: &str,
        _external_id: Option<&str>,
        _source_info: Option<&SourceInfo>,
    ) -> Result<AddOutcome> {
        if events.is_empty() {
            return Ok(AddOutcome::Added {
                first_sequence: 0,
                last_sequence: 0,
            });
        }

        let pk = Self::pk(domain, edition, root);

        // Validate sequence continuity
        let expected_next = self.get_next_sequence(domain, edition, root).await?;
        let first_seq = Self::get_sequence(&events[0]);

        if first_seq != expected_next {
            return Err(StorageError::SequenceConflict {
                expected: expected_next,
                actual: first_seq,
            });
        }

        let last_seq = events.last().map(Self::get_sequence).unwrap_or(first_seq);

        // Write events using batch write
        for event in &events {
            let seq = Self::get_sequence(event);
            let event_bytes = event.encode_to_vec();

            let mut item: HashMap<String, AttributeValue> = HashMap::new();
            item.insert("pk".to_string(), AttributeValue::S(pk.clone()));
            item.insert("seq".to_string(), AttributeValue::N(seq.to_string()));
            item.insert("event".to_string(), AttributeValue::B(event_bytes.into()));

            if let Some(ref ts) = event.created_at {
                let dt = chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_default();
                item.insert("created_at".to_string(), AttributeValue::S(dt));
            }

            if !correlation_id.is_empty() {
                item.insert(
                    "correlation_id".to_string(),
                    AttributeValue::S(correlation_id.to_string()),
                );
                // GSI sort key for correlation queries
                let gsi_sk = format!("{}#{}#{}#{}", domain, edition, root, seq);
                item.insert("gsi_sk".to_string(), AttributeValue::S(gsi_sk));
            }

            // Cascade tracking: extract from EventPage for GSI queries
            item.insert(
                "committed".to_string(),
                AttributeValue::Bool(!event.no_commit),
            );

            if let Some(ref cid) = event.cascade_id {
                item.insert("cascade_id".to_string(), AttributeValue::S(cid.clone()));
            }

            self.client
                .put_item()
                .table_name(&self.table_name)
                .set_item(Some(item))
                .send()
                .await
                .map_err(|e| {
                    StorageError::NotImplemented(format!("DynamoDB put_item failed: {}", e))
                })?;
        }

        debug!(
            domain = %domain,
            root = %root,
            count = events.len(),
            "Stored events in DynamoDB"
        );

        Ok(AddOutcome::Added {
            first_sequence: first_seq,
            last_sequence: last_seq,
        })
    }

    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Vec<EventPage>> {
        let pk = Self::pk(domain, edition, root);

        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("pk = :pk")
            .expression_attribute_values(":pk", AttributeValue::S(pk))
            .send()
            .await
            .map_err(|e| StorageError::NotImplemented(format!("DynamoDB query failed: {}", e)))?;

        let mut events = Vec::new();
        if let Some(items) = result.items {
            for item in items {
                if let Some(AttributeValue::B(blob)) = item.get("event") {
                    let event =
                        EventPage::decode(blob.as_ref()).map_err(StorageError::ProtobufDecode)?;
                    events.push(event);
                }
            }
        }

        Ok(events)
    }

    async fn get_from(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        if is_main_timeline(edition) {
            return self
                .query_edition_events(domain, DEFAULT_EDITION, root, from)
                .await;
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
        let pk = Self::pk(domain, edition, root);

        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("pk = :pk AND seq BETWEEN :from AND :to")
            .expression_attribute_values(":pk", AttributeValue::S(pk))
            .expression_attribute_values(":from", AttributeValue::N(from.to_string()))
            .expression_attribute_values(":to", AttributeValue::N((to - 1).to_string()))
            .send()
            .await
            .map_err(|e| StorageError::NotImplemented(format!("DynamoDB query failed: {}", e)))?;

        let mut events = Vec::new();
        if let Some(items) = result.items {
            for item in items {
                if let Some(AttributeValue::B(blob)) = item.get("event") {
                    let event =
                        EventPage::decode(blob.as_ref()).map_err(StorageError::ProtobufDecode)?;
                    events.push(event);
                }
            }
        }

        Ok(events)
    }

    async fn list_roots(&self, domain: &str, edition: &str) -> Result<Vec<Uuid>> {
        // Scan with filter - not efficient but DynamoDB doesn't support DISTINCT
        let prefix = format!("{}#{}#", domain, edition);

        let result = self
            .client
            .scan()
            .table_name(&self.table_name)
            .filter_expression("begins_with(pk, :prefix)")
            .expression_attribute_values(":prefix", AttributeValue::S(prefix))
            .projection_expression("pk")
            .send()
            .await
            .map_err(|e| StorageError::NotImplemented(format!("DynamoDB scan failed: {}", e)))?;

        let mut roots = std::collections::HashSet::new();
        if let Some(items) = result.items {
            for item in items {
                if let Some(AttributeValue::S(pk)) = item.get("pk") {
                    if let Some((_, _, root)) = Self::parse_pk(pk) {
                        roots.insert(root);
                    }
                }
            }
        }

        Ok(roots.into_iter().collect())
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        // Scan all items and extract unique domains
        let result = self
            .client
            .scan()
            .table_name(&self.table_name)
            .projection_expression("pk")
            .send()
            .await
            .map_err(|e| StorageError::NotImplemented(format!("DynamoDB scan failed: {}", e)))?;

        let mut domains = std::collections::HashSet::new();
        if let Some(items) = result.items {
            for item in items {
                if let Some(AttributeValue::S(pk)) = item.get("pk") {
                    if let Some((domain, _, _)) = Self::parse_pk(pk) {
                        domains.insert(domain);
                    }
                }
            }
        }

        Ok(domains.into_iter().collect())
    }

    async fn get_next_sequence(&self, domain: &str, edition: &str, root: Uuid) -> Result<u32> {
        if !is_main_timeline(edition) {
            let pk = Self::pk(domain, edition, root);

            let result = self
                .client
                .query()
                .table_name(&self.table_name)
                .key_condition_expression("pk = :pk")
                .expression_attribute_values(":pk", AttributeValue::S(pk))
                .scan_index_forward(false)
                .limit(1)
                .send()
                .await
                .map_err(|e| {
                    StorageError::NotImplemented(format!("DynamoDB query failed: {}", e))
                })?;

            if let Some(items) = result.items {
                if let Some(item) = items.first() {
                    if let Some(AttributeValue::N(seq_str)) = item.get("seq") {
                        if let Ok(seq) = seq_str.parse::<u32>() {
                            return Ok(seq + 1);
                        }
                    }
                }
            }
        }

        // Query main timeline
        let target_edition = if is_main_timeline(edition) {
            edition
        } else {
            DEFAULT_EDITION
        };

        let pk = Self::pk(domain, target_edition, root);

        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("pk = :pk")
            .expression_attribute_values(":pk", AttributeValue::S(pk))
            .scan_index_forward(false)
            .limit(1)
            .send()
            .await
            .map_err(|e| StorageError::NotImplemented(format!("DynamoDB query failed: {}", e)))?;

        if let Some(items) = result.items {
            if let Some(item) = items.first() {
                if let Some(AttributeValue::N(seq_str)) = item.get("seq") {
                    if let Ok(seq) = seq_str.parse::<u32>() {
                        return Ok(seq + 1);
                    }
                }
            }
        }

        Ok(0)
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

        // Query the GSI
        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("correlation-index")
            .key_condition_expression("correlation_id = :cid")
            .expression_attribute_values(":cid", AttributeValue::S(correlation_id.to_string()))
            .send()
            .await
            .map_err(|e| {
                StorageError::NotImplemented(format!("DynamoDB GSI query failed: {}", e))
            })?;

        // Group events by (domain, edition, root)
        let mut events_by_root: HashMap<(String, String, Uuid), Vec<EventPage>> = HashMap::new();

        if let Some(items) = result.items {
            for item in items {
                if let (Some(AttributeValue::S(pk)), Some(AttributeValue::B(blob))) =
                    (item.get("pk"), item.get("event"))
                {
                    if let Some((domain, edition, root)) = Self::parse_pk(pk) {
                        let event = EventPage::decode(blob.as_ref())
                            .map_err(StorageError::ProtobufDecode)?;
                        events_by_root
                            .entry((domain, edition, root))
                            .or_default()
                            .push(event);
                    }
                }
            }
        }

        // Build EventBooks
        let mut books = Vec::new();
        for ((domain, edition, root), mut pages) in events_by_root {
            pages.sort_by_key(Self::get_sequence);

            // Calculate next_sequence from pages
            let next_seq = pages.last().map(Self::get_sequence).unwrap_or(0) + 1;

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
                next_sequence: next_seq,
            });
        }

        Ok(books)
    }

    async fn delete_edition_events(&self, domain: &str, edition: &str) -> Result<u32> {
        let prefix = format!("{}#{}#", domain, edition);
        let mut deleted_count = 0u32;

        // Scan for matching items
        let result = self
            .client
            .scan()
            .table_name(&self.table_name)
            .filter_expression("begins_with(pk, :prefix)")
            .expression_attribute_values(":prefix", AttributeValue::S(prefix))
            .projection_expression("pk, seq")
            .send()
            .await
            .map_err(|e| StorageError::NotImplemented(format!("DynamoDB scan failed: {}", e)))?;

        if let Some(items) = result.items {
            for item in items {
                if let (Some(pk), Some(seq)) = (item.get("pk"), item.get("seq")) {
                    if let Err(e) = self
                        .client
                        .delete_item()
                        .table_name(&self.table_name)
                        .key("pk", pk.clone())
                        .key("seq", seq.clone())
                        .send()
                        .await
                    {
                        warn!(error = %e, "Failed to delete event from DynamoDB");
                    } else {
                        deleted_count += 1;
                    }
                }
            }
        }

        debug!(
            domain = %domain,
            edition = %edition,
            deleted = deleted_count,
            "Deleted edition events from DynamoDB"
        );

        Ok(deleted_count)
    }

    async fn find_by_source(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _source_info: &SourceInfo,
    ) -> Result<Option<Vec<EventPage>>> {
        // DynamoDB doesn't store source tracking - saga idempotency not supported
        // Use SQLite or PostgreSQL for saga source tracking
        Ok(None)
    }

    async fn find_by_external_id(
        &self,
        _domain: &str,
        _edition: &str,
        _root: Uuid,
        _external_id: &str,
    ) -> Result<Option<Vec<EventPage>>> {
        // DynamoDB doesn't store external_id tracking — fact pre-handler
        // idempotency not supported. Use SQLite or PostgreSQL.
        Ok(None)
    }

    async fn query_stale_cascades(&self, threshold: &str) -> Result<Vec<String>> {
        let threshold_dt = chrono::DateTime::parse_from_rfc3339(threshold)
            .map_err(|e| StorageError::InvalidTimestampFormat(e.to_string()))?;

        // Scan cascade-index to find all cascade_ids and their states
        // Group by cascade_id, check if any event is committed or all are stale
        let result = self
            .client
            .scan()
            .table_name(&self.table_name)
            .index_name("cascade-index")
            .projection_expression("cascade_id, committed, created_at")
            .send()
            .await
            .map_err(|e| {
                StorageError::NotImplemented(format!("DynamoDB cascade-index scan failed: {}", e))
            })?;

        // Track state per cascade_id
        struct CascadeState {
            has_committed: bool,
            all_before_threshold: bool,
        }
        let mut cascade_states: HashMap<String, CascadeState> = HashMap::new();

        if let Some(items) = result.items {
            for item in items {
                let cascade_id = match item.get("cascade_id") {
                    Some(AttributeValue::S(cid)) => cid.clone(),
                    _ => continue,
                };

                let committed = match item.get("committed") {
                    Some(AttributeValue::Bool(b)) => *b,
                    _ => false,
                };

                let is_stale = match item.get("created_at") {
                    Some(AttributeValue::S(ts)) => chrono::DateTime::parse_from_rfc3339(ts)
                        .map(|dt| dt < threshold_dt)
                        .unwrap_or(false),
                    _ => false,
                };

                let state = cascade_states.entry(cascade_id).or_insert(CascadeState {
                    has_committed: false,
                    all_before_threshold: true,
                });

                if committed {
                    state.has_committed = true;
                }
                if !is_stale {
                    state.all_before_threshold = false;
                }
            }
        }

        // Return cascade_ids that are stale (no committed events, all before threshold)
        Ok(cascade_states
            .into_iter()
            .filter(|(_, state)| !state.has_committed && state.all_before_threshold)
            .map(|(cid, _)| cid)
            .collect())
    }

    async fn query_cascade_participants(
        &self,
        cascade_id: &str,
    ) -> Result<Vec<CascadeParticipant>> {
        // Query cascade-index for all events with this cascade_id
        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("cascade-index")
            .key_condition_expression("cascade_id = :cid")
            .expression_attribute_values(":cid", AttributeValue::S(cascade_id.to_string()))
            .send()
            .await
            .map_err(|e| {
                StorageError::NotImplemented(format!("DynamoDB cascade-index query failed: {}", e))
            })?;

        // Group by (domain, edition, root), collect sequences for uncommitted events
        let mut participants_map: HashMap<(String, String, Uuid), Vec<u32>> = HashMap::new();

        if let Some(items) = result.items {
            for item in items {
                // Check if committed - skip committed events
                let committed = match item.get("committed") {
                    Some(AttributeValue::Bool(b)) => *b,
                    _ => false,
                };
                if committed {
                    continue;
                }

                // Parse pk to get domain, edition, root
                let pk = match item.get("pk") {
                    Some(AttributeValue::S(s)) => s,
                    _ => continue,
                };
                let (domain, edition, root) = match Self::parse_pk(pk) {
                    Some(parsed) => parsed,
                    None => continue,
                };

                // Get sequence
                let seq = match item.get("seq") {
                    Some(AttributeValue::N(s)) => s.parse::<u32>().unwrap_or(0),
                    _ => continue,
                };

                participants_map
                    .entry((domain, edition, root))
                    .or_default()
                    .push(seq);
            }
        }

        // Convert to CascadeParticipant list
        Ok(participants_map
            .into_iter()
            .map(|((domain, edition, root), sequences)| CascadeParticipant {
                domain,
                edition,
                root,
                sequences,
            })
            .collect())
    }
}
