//! EventStoreDB implementations of storage interfaces.

use async_trait::async_trait;
use eventstore::{
    AppendToStreamOptions, Client, ClientSettings, EventData, ExpectedRevision, ReadStreamOptions,
    StreamPosition,
};
use prost::bytes::Bytes;
use prost::Message;
use uuid::Uuid;

use super::{EventStore, Result, SnapshotStore, StorageError};
use crate::proto::{EventPage, Snapshot};

/// EventStoreDB implementation of EventStore.
///
/// Uses stream-per-aggregate pattern: each aggregate root has its own stream
/// named `{domain}-{root}`.
pub struct EventStoreDbEventStore {
    client: Client,
}

impl EventStoreDbEventStore {
    /// Create a new EventStoreDB event store.
    pub async fn new(connection_string: &str) -> Result<Self> {
        let settings = connection_string
            .parse::<ClientSettings>()
            .map_err(|e| StorageError::EventStoreDb(e.to_string()))?;
        let client =
            Client::new(settings).map_err(|e| StorageError::EventStoreDb(e.to_string()))?;
        Ok(Self { client })
    }

    /// Get stream name for an aggregate.
    fn stream_name(domain: &str, root: Uuid) -> String {
        format!("{}-{}", domain, root)
    }
}

#[async_trait]
impl EventStore for EventStoreDbEventStore {
    async fn add(&self, domain: &str, root: Uuid, events: Vec<EventPage>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let stream = Self::stream_name(domain, root);

        // Get current stream revision for optimistic concurrency
        let current_revision = self.get_next_sequence(domain, root).await?;
        let expected = if current_revision == 0 {
            ExpectedRevision::NoStream
        } else {
            ExpectedRevision::Exact((current_revision - 1) as u64)
        };

        // Convert events to EventStoreDB format
        let event_data: Vec<EventData> = events
            .iter()
            .map(|event| {
                let event_type = event
                    .event
                    .as_ref()
                    .map(|e| e.type_url.rsplit('/').next().unwrap_or("UnknownEvent"))
                    .unwrap_or("UnknownEvent");

                let data = Bytes::from(event.encode_to_vec());

                EventData::binary(event_type.to_string(), data)
            })
            .collect();

        let options = AppendToStreamOptions::default().expected_revision(expected);

        self.client
            .append_to_stream(stream, &options, event_data)
            .await
            .map_err(|e| {
                // Check for wrong expected version (sequence conflict)
                let err_str = e.to_string();
                if err_str.contains("WrongExpectedVersion") {
                    StorageError::SequenceConflict {
                        expected: current_revision,
                        actual: current_revision + 1, // Approximate
                    }
                } else {
                    StorageError::EventStoreDb(err_str)
                }
            })?;

        Ok(())
    }

    async fn get(&self, domain: &str, root: Uuid) -> Result<Vec<EventPage>> {
        self.get_from(domain, root, 0).await
    }

    async fn get_from(&self, domain: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>> {
        let stream = Self::stream_name(domain, root);

        let options = ReadStreamOptions::default().position(StreamPosition::Position(from as u64));

        let mut stream_result = match self.client.read_stream(stream, &options).await {
            Ok(s) => s,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("StreamNotFound") || err_str.contains("stream not found") {
                    return Ok(Vec::new());
                }
                return Err(StorageError::EventStoreDb(err_str));
            }
        };

        let mut events = Vec::new();
        loop {
            match stream_result.next().await {
                Ok(Some(event)) => {
                    let data = event.get_original_event().data.as_ref();
                    let event_page = EventPage::decode(data)?;
                    events.push(event_page);
                }
                Ok(None) => break,
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("StreamNotFound") || err_str.contains("stream not found") {
                        break;
                    }
                    return Err(StorageError::EventStoreDb(err_str));
                }
            }
        }

        Ok(events)
    }

    async fn get_from_to(
        &self,
        domain: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>> {
        let stream = Self::stream_name(domain, root);
        let count = to.saturating_sub(from) as usize;

        let options = ReadStreamOptions::default()
            .position(StreamPosition::Position(from as u64))
            .max_count(count);

        let mut stream_result = match self.client.read_stream(stream, &options).await {
            Ok(s) => s,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("StreamNotFound") || err_str.contains("stream not found") {
                    return Ok(Vec::new());
                }
                return Err(StorageError::EventStoreDb(err_str));
            }
        };

        let mut events = Vec::new();
        loop {
            match stream_result.next().await {
                Ok(Some(event)) => {
                    let data = event.get_original_event().data.as_ref();
                    let event_page = EventPage::decode(data)?;
                    events.push(event_page);
                }
                Ok(None) => break,
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("StreamNotFound") || err_str.contains("stream not found") {
                        break;
                    }
                    return Err(StorageError::EventStoreDb(err_str));
                }
            }
        }

        Ok(events)
    }

    async fn list_roots(&self, domain: &str) -> Result<Vec<Uuid>> {
        // Read $streams system stream and filter by domain prefix
        let options = ReadStreamOptions::default().position(StreamPosition::Start);

        let mut stream_result = self
            .client
            .read_stream("$streams", &options)
            .await
            .map_err(|e| StorageError::EventStoreDb(e.to_string()))?;

        let prefix = format!("{}-", domain);
        let mut roots = Vec::new();

        loop {
            match stream_result.next().await {
                Ok(Some(event)) => {
                    let stream_name = event.get_original_event().stream_id.as_str();
                    if let Some(suffix) = stream_name.strip_prefix(&prefix) {
                        // Skip snapshot streams
                        if !suffix.ends_with("-snapshot") {
                            if let Ok(uuid) = Uuid::parse_str(suffix) {
                                roots.push(uuid);
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    return Err(StorageError::EventStoreDb(e.to_string()));
                }
            }
        }

        Ok(roots)
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        // Read $streams system stream and extract unique domain prefixes
        let options = ReadStreamOptions::default().position(StreamPosition::Start);

        let mut stream_result = self
            .client
            .read_stream("$streams", &options)
            .await
            .map_err(|e| StorageError::EventStoreDb(e.to_string()))?;

        let mut domains = std::collections::HashSet::new();

        loop {
            match stream_result.next().await {
                Ok(Some(event)) => {
                    let stream_name = event.get_original_event().stream_id.as_str();
                    // Extract domain from stream name (everything before first UUID)
                    if let Some(pos) = stream_name.find('-') {
                        let potential_domain = &stream_name[..pos];
                        // Skip system streams (start with $)
                        if !potential_domain.starts_with('$') {
                            domains.insert(potential_domain.to_string());
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    return Err(StorageError::EventStoreDb(e.to_string()));
                }
            }
        }

        Ok(domains.into_iter().collect())
    }

    async fn get_next_sequence(&self, domain: &str, root: Uuid) -> Result<u32> {
        let stream = Self::stream_name(domain, root);

        let options = ReadStreamOptions::default()
            .position(StreamPosition::End)
            .backwards()
            .max_count(1);

        let mut stream_result = self
            .client
            .read_stream(stream, &options)
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                // Stream not found means sequence 0
                if err_str.contains("StreamNotFound") {
                    return StorageError::NotFound {
                        domain: domain.to_string(),
                        root,
                    };
                }
                StorageError::EventStoreDb(err_str)
            })?;

        match stream_result.next().await {
            Ok(Some(event)) => Ok(event.get_original_event().revision as u32 + 1),
            Ok(None) => Ok(0),
            Err(e) => Err(StorageError::EventStoreDb(e.to_string())),
        }
    }
}

/// EventStoreDB implementation of SnapshotStore.
///
/// Stores snapshots in a separate stream `{domain}-{root}-snapshot` with a single event.
pub struct EventStoreDbSnapshotStore {
    client: Client,
}

impl EventStoreDbSnapshotStore {
    /// Create a new EventStoreDB snapshot store.
    pub async fn new(connection_string: &str) -> Result<Self> {
        let settings = connection_string
            .parse::<ClientSettings>()
            .map_err(|e| StorageError::EventStoreDb(e.to_string()))?;
        let client =
            Client::new(settings).map_err(|e| StorageError::EventStoreDb(e.to_string()))?;
        Ok(Self { client })
    }

    /// Get snapshot stream name for an aggregate.
    fn snapshot_stream(domain: &str, root: Uuid) -> String {
        format!("{}-{}-snapshot", domain, root)
    }
}

#[async_trait]
impl SnapshotStore for EventStoreDbSnapshotStore {
    async fn get(&self, domain: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let stream = Self::snapshot_stream(domain, root);

        let options = ReadStreamOptions::default()
            .position(StreamPosition::End)
            .backwards()
            .max_count(1);

        let mut stream_result = match self.client.read_stream(stream, &options).await {
            Ok(s) => s,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("StreamNotFound") {
                    return Ok(None);
                }
                return Err(StorageError::EventStoreDb(err_str));
            }
        };

        match stream_result.next().await {
            Ok(Some(event)) => {
                let data = event.get_original_event().data.as_ref();
                let snapshot = Snapshot::decode(data)?;
                Ok(Some(snapshot))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(StorageError::EventStoreDb(e.to_string())),
        }
    }

    async fn put(&self, domain: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let stream = Self::snapshot_stream(domain, root);
        let data = Bytes::from(snapshot.encode_to_vec());

        let event = EventData::binary("Snapshot".to_string(), data);

        // Use Any revision since we're replacing the snapshot
        let options = AppendToStreamOptions::default().expected_revision(ExpectedRevision::Any);

        self.client
            .append_to_stream(stream, &options, vec![event])
            .await
            .map_err(|e| StorageError::EventStoreDb(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, domain: &str, root: Uuid) -> Result<()> {
        let stream = Self::snapshot_stream(domain, root);

        // Soft delete the stream
        self.client
            .delete_stream(stream, &Default::default())
            .await
            .map_err(|e| StorageError::EventStoreDb(e.to_string()))?;

        Ok(())
    }
}
