//! Bigtable EventStore implementation.
//!
//! Row key format: `{domain}#{edition}#{root}#{sequence:010}`
//! Column family: `event`
//! Columns: `data` (EventPage), `created_at` (timestamp), `correlation_id`
//!
//! Note: This implementation requires a Bigtable emulator or real Bigtable instance.
//! Tables must be pre-created with the `event` column family.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bigtable_rs::bigtable::{BigTable, BigTableConnection};
use bigtable_rs::google::bigtable::v2::mutation::SetCell;
use bigtable_rs::google::bigtable::v2::row_filter::Filter;
use bigtable_rs::google::bigtable::v2::{
    MutateRowRequest, Mutation, ReadRowsRequest, RowFilter, RowRange, RowSet,
};
use prost::Message;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::proto::{Cover, Edition, EventBook, EventPage, Uuid as ProtoUuid};
use crate::storage::helpers::is_main_timeline;
use crate::storage::{EventStore, Result, StorageError};

const COLUMN_FAMILY: &str = "event";
const COL_DATA: &[u8] = b"data";
const COL_CREATED_AT: &[u8] = b"created_at";
const COL_CORRELATION_ID: &[u8] = b"correlation_id";

/// Bigtable implementation of EventStore.
///
/// Row key format: `{domain}#{edition}#{root}#{sequence:010}`
pub struct BigtableEventStore {
    client: Arc<Mutex<BigTable>>,
    table_name: String,
}

impl BigtableEventStore {
    /// Create a new Bigtable event store.
    pub async fn new(
        project_id: &str,
        instance_id: &str,
        table_name: impl Into<String>,
        emulator_host: Option<&str>,
    ) -> Result<Self> {
        let connection = if let Some(host) = emulator_host {
            BigTableConnection::new_with_emulator(host, project_id, instance_id, false, None)
                .map_err(|e| {
                    StorageError::NotImplemented(format!(
                        "Bigtable emulator connection failed: {}",
                        e
                    ))
                })?
        } else {
            BigTableConnection::new(
                project_id,
                instance_id,
                false,
                1,
                Some(Duration::from_secs(30)),
            )
            .await
            .map_err(|e| {
                StorageError::NotImplemented(format!("Bigtable connection failed: {}", e))
            })?
        };

        let client = Arc::new(Mutex::new(connection.client()));
        let table_name = table_name.into();

        info!(
            project = %project_id,
            instance = %instance_id,
            table = %table_name,
            "Connected to Bigtable for events"
        );

        Ok(Self { client, table_name })
    }

    /// Build the row key for an event.
    pub fn row_key(domain: &str, edition: &str, root: Uuid, sequence: u32) -> Vec<u8> {
        format!("{}#{}#{}#{:010}", domain, edition, root, sequence).into_bytes()
    }

    /// Build the row key prefix for scanning all events of a root.
    pub fn row_key_prefix(domain: &str, edition: &str, root: Uuid) -> Vec<u8> {
        format!("{}#{}#{}#", domain, edition, root).into_bytes()
    }

    /// Parse row key into (domain, edition, root, sequence).
    pub fn parse_row_key(key: &[u8]) -> Option<(String, String, Uuid, u32)> {
        let key_str = String::from_utf8(key.to_vec()).ok()?;
        let parts: Vec<&str> = key_str.splitn(4, '#').collect();

        if parts.len() != 4 {
            return None;
        }

        let domain = parts[0].to_string();
        let edition = parts[1].to_string();
        let root = Uuid::parse_str(parts[2]).ok()?;
        let sequence = parts[3].parse::<u32>().ok()?;

        Some((domain, edition, root, sequence))
    }

    /// Get sequence from EventPage.
    pub fn get_sequence(event: &EventPage) -> u32 {
        event.sequence
    }

    /// Parse ISO 8601 timestamp string to (seconds, nanos).
    pub fn parse_timestamp(ts: &str) -> Option<(i64, i32)> {
        chrono::DateTime::parse_from_rfc3339(ts)
            .ok()
            .map(|dt| (dt.timestamp(), dt.timestamp_subsec_nanos() as i32))
    }

    /// Format timestamp to ISO 8601 string.
    pub fn format_timestamp(seconds: i64, nanos: i32) -> String {
        chrono::DateTime::from_timestamp(seconds, nanos as u32)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default()
    }

    /// Build a SetCell mutation.
    pub fn build_set_cell(family: &str, qualifier: &[u8], value: &[u8]) -> Mutation {
        Mutation {
            mutation: Some(
                bigtable_rs::google::bigtable::v2::mutation::Mutation::SetCell(SetCell {
                    family_name: family.to_string(),
                    column_qualifier: qualifier.to_vec(),
                    timestamp_micros: -1, // Server timestamp
                    value: value.to_vec(),
                }),
            ),
        }
    }

    /// Build mutations for an event.
    pub fn build_event_mutations(event: &EventPage, correlation_id: &str) -> Vec<Mutation> {
        let mut mutations = Vec::new();

        // Event data
        mutations.push(Self::build_set_cell(
            COLUMN_FAMILY,
            COL_DATA,
            &event.encode_to_vec(),
        ));

        // Created at timestamp
        if let Some(ref ts) = event.created_at {
            let ts_str = Self::format_timestamp(ts.seconds, ts.nanos);
            mutations.push(Self::build_set_cell(
                COLUMN_FAMILY,
                COL_CREATED_AT,
                ts_str.as_bytes(),
            ));
        }

        // Correlation ID
        if !correlation_id.is_empty() {
            mutations.push(Self::build_set_cell(
                COLUMN_FAMILY,
                COL_CORRELATION_ID,
                correlation_id.as_bytes(),
            ));
        }

        mutations
    }

    /// Query events for a specific edition starting from a sequence.
    async fn query_edition_events(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        let start_key = Self::row_key(domain, edition, root, from);
        let end_key = Self::row_key(domain, edition, root, u32::MAX);

        let mut client = self.client.lock().await;
        let table_name = client.get_full_table_name(&self.table_name);

        let request = ReadRowsRequest {
            table_name,
            rows: Some(RowSet {
                row_keys: vec![],
                row_ranges: vec![RowRange {
                    start_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::StartKey::StartKeyClosed(
                            start_key,
                        ),
                    ),
                    end_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::EndKey::EndKeyClosed(end_key),
                    ),
                }],
            }),
            filter: Some(RowFilter {
                filter: Some(Filter::FamilyNameRegexFilter(COLUMN_FAMILY.to_string())),
            }),
            ..Default::default()
        };

        let result = client.read_rows(request).await.map_err(|e| {
            StorageError::NotImplemented(format!("Bigtable read_rows failed: {}", e))
        })?;

        let mut events = Vec::new();
        for (_, cells) in result {
            for cell in cells {
                if cell.qualifier == COL_DATA {
                    let event = EventPage::decode(cell.value.as_ref())
                        .map_err(StorageError::ProtobufDecode)?;
                    events.push(event);
                }
            }
        }

        events.sort_by_key(Self::get_sequence);
        Ok(events)
    }

    /// Get minimum sequence from edition events (divergence point).
    async fn get_edition_min_sequence(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
    ) -> Result<Option<u32>> {
        let prefix = Self::row_key_prefix(domain, edition, root);

        let mut client = self.client.lock().await;
        let table_name = client.get_full_table_name(&self.table_name);

        let request = ReadRowsRequest {
            table_name,
            rows: Some(RowSet {
                row_keys: vec![],
                row_ranges: vec![RowRange {
                    start_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::StartKey::StartKeyClosed(
                            prefix.clone(),
                        ),
                    ),
                    end_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::EndKey::EndKeyOpen({
                            let mut end = prefix;
                            if let Some(last) = end.last_mut() {
                                *last = last.saturating_add(1);
                            }
                            end
                        }),
                    ),
                }],
            }),
            rows_limit: 1,
            ..Default::default()
        };

        let result = client.read_rows(request).await.map_err(|e| {
            StorageError::NotImplemented(format!("Bigtable read_rows failed: {}", e))
        })?;

        for (row_key, _) in result {
            if let Some((_, _, _, seq)) = Self::parse_row_key(&row_key) {
                return Ok(Some(seq));
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

        let start_key = Self::row_key(domain, DEFAULT_EDITION, root, from);
        let end_key = Self::row_key(domain, DEFAULT_EDITION, root, until_seq - 1);

        let mut client = self.client.lock().await;
        let table_name = client.get_full_table_name(&self.table_name);

        let request = ReadRowsRequest {
            table_name,
            rows: Some(RowSet {
                row_keys: vec![],
                row_ranges: vec![RowRange {
                    start_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::StartKey::StartKeyClosed(
                            start_key,
                        ),
                    ),
                    end_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::EndKey::EndKeyClosed(end_key),
                    ),
                }],
            }),
            filter: Some(RowFilter {
                filter: Some(Filter::FamilyNameRegexFilter(COLUMN_FAMILY.to_string())),
            }),
            ..Default::default()
        };

        let result = client.read_rows(request).await.map_err(|e| {
            StorageError::NotImplemented(format!("Bigtable read_rows failed: {}", e))
        })?;

        let mut events = Vec::new();
        for (_, cells) in result {
            for cell in cells {
                if cell.qualifier == COL_DATA {
                    let event = EventPage::decode(cell.value.as_ref())
                        .map_err(StorageError::ProtobufDecode)?;
                    events.push(event);
                }
            }
        }

        events.sort_by_key(Self::get_sequence);
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

    /// Get maximum sequence number for an edition.
    async fn get_max_sequence_for_edition(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
    ) -> Result<Option<u32>> {
        let prefix = Self::row_key_prefix(domain, edition, root);

        let mut client = self.client.lock().await;
        let table_name = client.get_full_table_name(&self.table_name);

        let request = ReadRowsRequest {
            table_name,
            rows: Some(RowSet {
                row_keys: vec![],
                row_ranges: vec![RowRange {
                    start_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::StartKey::StartKeyClosed(
                            prefix.clone(),
                        ),
                    ),
                    end_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::EndKey::EndKeyOpen({
                            let mut end = prefix;
                            if let Some(last) = end.last_mut() {
                                *last = last.saturating_add(1);
                            }
                            end
                        }),
                    ),
                }],
            }),
            ..Default::default()
        };

        let result = client.read_rows(request).await.map_err(|e| {
            StorageError::NotImplemented(format!("Bigtable read_rows failed: {}", e))
        })?;

        let mut max_seq: Option<u32> = None;
        for (row_key, _) in result {
            if let Some((_, _, _, seq)) = Self::parse_row_key(&row_key) {
                max_seq = Some(max_seq.map_or(seq, |m| m.max(seq)));
            }
        }

        Ok(max_seq)
    }
}

#[async_trait]
impl EventStore for BigtableEventStore {
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

        // Validate sequence continuity
        let expected_next = self.get_next_sequence(domain, edition, root).await?;
        let first_seq = Self::get_sequence(&events[0]);

        if first_seq != expected_next {
            return Err(StorageError::SequenceConflict {
                expected: expected_next,
                actual: first_seq,
            });
        }

        let mut client = self.client.lock().await;
        let table_name = client.get_full_table_name(&self.table_name);

        for event in &events {
            let seq = Self::get_sequence(event);
            let row_key = Self::row_key(domain, edition, root, seq);
            let mutations = Self::build_event_mutations(event, correlation_id);

            let request = MutateRowRequest {
                table_name: table_name.clone(),
                row_key,
                mutations,
                ..Default::default()
            };

            client.mutate_row(request).await.map_err(|e| {
                StorageError::NotImplemented(format!("Bigtable mutate_row failed: {}", e))
            })?;
        }

        debug!(
            domain = %domain,
            root = %root,
            count = events.len(),
            "Stored events in Bigtable"
        );

        Ok(())
    }

    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Vec<EventPage>> {
        self.query_edition_events(domain, edition, root, 0).await
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
        let start_key = Self::row_key(domain, edition, root, from);
        let end_key = Self::row_key(domain, edition, root, to.saturating_sub(1));

        let mut client = self.client.lock().await;
        let table_name = client.get_full_table_name(&self.table_name);

        let request = ReadRowsRequest {
            table_name,
            rows: Some(RowSet {
                row_keys: vec![],
                row_ranges: vec![RowRange {
                    start_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::StartKey::StartKeyClosed(
                            start_key,
                        ),
                    ),
                    end_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::EndKey::EndKeyClosed(end_key),
                    ),
                }],
            }),
            filter: Some(RowFilter {
                filter: Some(Filter::FamilyNameRegexFilter(COLUMN_FAMILY.to_string())),
            }),
            ..Default::default()
        };

        let result = client.read_rows(request).await.map_err(|e| {
            StorageError::NotImplemented(format!("Bigtable read_rows failed: {}", e))
        })?;

        let mut events = Vec::new();
        for (_, cells) in result {
            for cell in cells {
                if cell.qualifier == COL_DATA {
                    let event = EventPage::decode(cell.value.as_ref())
                        .map_err(StorageError::ProtobufDecode)?;
                    events.push(event);
                }
            }
        }

        events.sort_by_key(Self::get_sequence);
        Ok(events)
    }

    async fn list_roots(&self, domain: &str, edition: &str) -> Result<Vec<Uuid>> {
        let prefix = format!("{}#{}#", domain, edition).into_bytes();

        let mut client = self.client.lock().await;
        let table_name = client.get_full_table_name(&self.table_name);

        let request = ReadRowsRequest {
            table_name,
            rows: Some(RowSet {
                row_keys: vec![],
                row_ranges: vec![RowRange {
                    start_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::StartKey::StartKeyClosed(
                            prefix.clone(),
                        ),
                    ),
                    end_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::EndKey::EndKeyOpen({
                            let mut end = prefix;
                            if let Some(last) = end.last_mut() {
                                *last = last.saturating_add(1);
                            }
                            end
                        }),
                    ),
                }],
            }),
            ..Default::default()
        };

        let result = client.read_rows(request).await.map_err(|e| {
            StorageError::NotImplemented(format!("Bigtable read_rows failed: {}", e))
        })?;

        let mut roots = std::collections::HashSet::new();
        for (row_key, _) in result {
            if let Some((_, _, root, _)) = Self::parse_row_key(&row_key) {
                roots.insert(root);
            }
        }

        Ok(roots.into_iter().collect())
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        let mut client = self.client.lock().await;
        let table_name = client.get_full_table_name(&self.table_name);

        let request = ReadRowsRequest {
            table_name,
            ..Default::default()
        };

        let result = client.read_rows(request).await.map_err(|e| {
            StorageError::NotImplemented(format!("Bigtable read_rows failed: {}", e))
        })?;

        let mut domains = std::collections::HashSet::new();
        for (row_key, _) in result {
            if let Some((domain, _, _, _)) = Self::parse_row_key(&row_key) {
                domains.insert(domain);
            }
        }

        Ok(domains.into_iter().collect())
    }

    async fn get_next_sequence(&self, domain: &str, edition: &str, root: Uuid) -> Result<u32> {
        if !is_main_timeline(edition) {
            if let Some(seq) = self
                .get_max_sequence_for_edition(domain, edition, root)
                .await?
            {
                return Ok(seq + 1);
            }
        }

        let target_edition = if is_main_timeline(edition) {
            edition
        } else {
            DEFAULT_EDITION
        };

        if let Some(seq) = self
            .get_max_sequence_for_edition(domain, target_edition, root)
            .await?
        {
            return Ok(seq + 1);
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

        warn!(
            correlation_id = %correlation_id,
            "get_by_correlation requires full table scan in Bigtable - consider using a separate index table"
        );

        let mut client = self.client.lock().await;
        let table_name = client.get_full_table_name(&self.table_name);

        let request = ReadRowsRequest {
            table_name,
            filter: Some(RowFilter {
                filter: Some(Filter::FamilyNameRegexFilter(COLUMN_FAMILY.to_string())),
            }),
            ..Default::default()
        };

        let result = client.read_rows(request).await.map_err(|e| {
            StorageError::NotImplemented(format!("Bigtable read_rows failed: {}", e))
        })?;

        let mut events_by_root: HashMap<(String, String, Uuid), Vec<EventPage>> = HashMap::new();

        for (row_key, cells) in result {
            let mut event_data: Option<Vec<u8>> = None;
            let mut row_correlation_id: Option<String> = None;

            for cell in cells {
                if cell.qualifier == COL_DATA {
                    event_data = Some(cell.value);
                } else if cell.qualifier == COL_CORRELATION_ID {
                    row_correlation_id = String::from_utf8(cell.value).ok();
                }
            }

            if row_correlation_id.as_deref() == Some(correlation_id) {
                if let (Some(data), Some((domain, edition, root, _))) =
                    (event_data, Self::parse_row_key(&row_key))
                {
                    let event =
                        EventPage::decode(data.as_ref()).map_err(StorageError::ProtobufDecode)?;
                    events_by_root
                        .entry((domain, edition, root))
                        .or_default()
                        .push(event);
                }
            }
        }

        let mut books = Vec::new();
        for ((domain, edition, root), mut pages) in events_by_root {
            pages.sort_by_key(Self::get_sequence);

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
        let prefix = format!("{}#{}#", domain, edition).into_bytes();

        let mut client = self.client.lock().await;
        let table_name = client.get_full_table_name(&self.table_name);

        let request = ReadRowsRequest {
            table_name: table_name.clone(),
            rows: Some(RowSet {
                row_keys: vec![],
                row_ranges: vec![RowRange {
                    start_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::StartKey::StartKeyClosed(
                            prefix.clone(),
                        ),
                    ),
                    end_key: Some(
                        bigtable_rs::google::bigtable::v2::row_range::EndKey::EndKeyOpen({
                            let mut end = prefix;
                            if let Some(last) = end.last_mut() {
                                *last = last.saturating_add(1);
                            }
                            end
                        }),
                    ),
                }],
            }),
            ..Default::default()
        };

        let result = client.read_rows(request).await.map_err(|e| {
            StorageError::NotImplemented(format!("Bigtable read_rows failed: {}", e))
        })?;

        let mut deleted_count = 0u32;

        for (row_key, _) in result {
            let delete_mutation = Mutation {
                mutation: Some(
                    bigtable_rs::google::bigtable::v2::mutation::Mutation::DeleteFromRow(
                        bigtable_rs::google::bigtable::v2::mutation::DeleteFromRow {},
                    ),
                ),
            };

            let delete_request = MutateRowRequest {
                table_name: table_name.clone(),
                row_key,
                mutations: vec![delete_mutation],
                ..Default::default()
            };

            if let Err(e) = client.mutate_row(delete_request).await {
                warn!(error = %e, "Failed to delete row from Bigtable");
            } else {
                deleted_count += 1;
            }
        }

        debug!(
            domain = %domain,
            edition = %edition,
            deleted = deleted_count,
            "Deleted edition events from Bigtable"
        );

        Ok(deleted_count)
    }
}
