//! MongoDB EventStore implementation.

use async_trait::async_trait;
use mongodb::bson::{doc, Binary, Bson};
use mongodb::options::{FindOptions, IndexOptions};
use mongodb::{Client, Collection, Database, IndexModel};
use prost::Message;
use uuid::Uuid;

use crate::storage::{EventStore, Result, StorageError};
use crate::proto::EventPage;

use super::{EVENTS_COLLECTION};

/// MongoDB implementation of EventStore.
pub struct MongoEventStore {
    database: Database,
    events: Collection<mongodb::bson::Document>,
}

impl MongoEventStore {
    /// Create a new MongoDB event store.
    pub async fn new(client: &Client, database_name: &str) -> Result<Self> {
        let database = client.database(database_name);
        let events = database.collection(EVENTS_COLLECTION);

        let store = Self { database, events };
        store.init().await?;

        Ok(store)
    }

    /// Initialize indexes for optimal query performance.
    async fn init(&self) -> Result<()> {
        // Compound unique index on (domain, root, sequence)
        let index = IndexModel::builder()
            .keys(doc! { "edition": 1, "domain": 1, "root": 1, "sequence": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        self.events.create_index(index).await?;

        // Index for listing roots by domain
        let domain_index = IndexModel::builder().keys(doc! { "edition": 1, "domain": 1 }).build();

        self.events.create_index(domain_index).await?;

        // Index for temporal queries (created_at filtering)
        let temporal_index = IndexModel::builder()
            .keys(doc! { "edition": 1, "domain": 1, "root": 1, "created_at": 1 })
            .build();

        self.events.create_index(temporal_index).await?;

        Ok(())
    }

    /// Get the database reference for transaction support.
    pub fn database(&self) -> &Database {
        &self.database
    }
}

#[async_trait]
impl EventStore for MongoEventStore {
    async fn add(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        events: Vec<EventPage>,
        _correlation_id: &str,
    ) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let root_str = root.to_string();

        // Get the next sequence number (no transaction needed - unique index handles conflicts)
        let base_sequence = {
            let filter = doc! { "edition": edition, "domain": domain, "root": &root_str };
            let options = FindOptions::builder()
                .sort(doc! { "sequence": -1 })
                .limit(1)
                .build();

            let mut cursor = self.events.find(filter).with_options(options).await?;

            if cursor.advance().await? {
                let doc = cursor.deserialize_current()?;
                doc.get_i32("sequence").unwrap_or(0) as u32 + 1
            } else {
                0
            }
        };

        let mut auto_sequence = base_sequence;

        for event in events {
            let event_data = event.encode_to_vec();
            let sequence =
                crate::storage::helpers::resolve_sequence(&event, base_sequence, &mut auto_sequence)?;
            let created_at = crate::storage::helpers::parse_timestamp(&event)?;

            let doc = doc! {
                "edition": edition,
                "domain": domain,
                "root": &root_str,
                "sequence": sequence as i32,
                "created_at": &created_at,
                "event_data": Binary { subtype: mongodb::bson::spec::BinarySubtype::Generic, bytes: event_data },
            };

            // Insert with unique index enforcing consistency
            // Duplicate key error indicates concurrent write conflict
            self.events.insert_one(doc).await.map_err(|e| {
                if let mongodb::error::ErrorKind::Write(mongodb::error::WriteFailure::WriteError(
                    ref write_err,
                )) = *e.kind
                {
                    if write_err.code == 11000 {
                        // Duplicate key error - sequence conflict
                        return StorageError::SequenceConflict {
                            expected: sequence,
                            actual: sequence,
                        };
                    }
                }
                StorageError::from(e)
            })?;
        }

        Ok(())
    }

    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Vec<EventPage>> {
        self.get_from(domain, edition, root, 0).await
    }

    async fn get_from(&self, domain: &str, edition: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>> {
        let root_str = root.to_string();

        let filter = doc! {
            "edition": edition,
            "domain": domain,
            "root": &root_str,
            "sequence": { "$gte": from as i32 }
        };

        let options = FindOptions::builder().sort(doc! { "sequence": 1 }).build();

        tracing::info!(
            domain = domain,
            root = %root_str,
            from = from,
            collection = %self.events.name(),
            database = %self.database.name(),
            "MongoDB get_from query starting"
        );

        let mut cursor = self.events.find(filter).with_options(options).await?;

        tracing::info!("MongoDB cursor created");

        let mut events = Vec::new();
        let mut doc_count = 0;
        while cursor.advance().await? {
            doc_count += 1;
            let doc = cursor.deserialize_current()?;
            tracing::info!(doc_count, "Processing document from cursor");
            let event_data =
                doc.get_binary_generic("event_data")
                    .map_err(|_| StorageError::NotFound {
                        domain: domain.to_string(),
                        root,
                    })?;
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        tracing::info!(
            domain = domain,
            root = %root_str,
            doc_count,
            events_len = events.len(),
            "MongoDB get_from completed"
        );

        Ok(events)
    }

    async fn get_from_to(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>> {
        // If to is u32::MAX or would overflow i32, use unbounded upper query
        // This prevents i32 overflow (u32::MAX as i32 = -1)
        if to > i32::MAX as u32 {
            return self.get_from(domain, edition, root, from).await;
        }

        let root_str = root.to_string();

        let filter = doc! {
            "edition": edition,
            "domain": domain,
            "root": &root_str,
            "sequence": { "$gte": from as i32, "$lt": to as i32 }
        };

        let options = FindOptions::builder().sort(doc! { "sequence": 1 }).build();

        let mut cursor = self.events.find(filter).with_options(options).await?;

        let mut events = Vec::new();
        while cursor.advance().await? {
            let doc = cursor.deserialize_current()?;
            let event_data =
                doc.get_binary_generic("event_data")
                    .map_err(|_| StorageError::NotFound {
                        domain: domain.to_string(),
                        root,
                    })?;
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    async fn get_until_timestamp(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        until: &str,
    ) -> Result<Vec<EventPage>> {
        let root_str = root.to_string();

        let filter = doc! {
            "edition": edition,
            "domain": domain,
            "root": &root_str,
            "created_at": { "$lte": until }
        };

        let options = FindOptions::builder().sort(doc! { "sequence": 1 }).build();

        let mut cursor = self.events.find(filter).with_options(options).await?;

        let mut events = Vec::new();
        while cursor.advance().await? {
            let doc = cursor.deserialize_current()?;
            let event_data =
                doc.get_binary_generic("event_data")
                    .map_err(|_| StorageError::NotFound {
                        domain: domain.to_string(),
                        root,
                    })?;
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    async fn list_roots(&self, domain: &str, edition: &str) -> Result<Vec<Uuid>> {
        let pipeline = vec![
            doc! { "$match": { "edition": edition, "domain": domain } },
            doc! { "$group": { "_id": "$root" } },
        ];

        let mut cursor = self.events.aggregate(pipeline).await?;

        let mut roots = Vec::new();
        while cursor.advance().await? {
            let doc = cursor.deserialize_current()?;
            if let Some(Bson::String(root_str)) = doc.get("_id") {
                let root = Uuid::parse_str(root_str)?;
                roots.push(root);
            }
        }

        Ok(roots)
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        let pipeline = vec![doc! { "$group": { "_id": "$domain" } }];

        let mut cursor = self.events.aggregate(pipeline).await?;

        let mut domains = Vec::new();
        while cursor.advance().await? {
            let doc = cursor.deserialize_current()?;
            if let Some(Bson::String(domain)) = doc.get("_id") {
                domains.push(domain.clone());
            }
        }

        Ok(domains)
    }

    async fn get_next_sequence(&self, domain: &str, edition: &str, root: Uuid) -> Result<u32> {
        let root_str = root.to_string();

        let filter = doc! { "edition": edition, "domain": domain, "root": &root_str };
        let options = FindOptions::builder()
            .sort(doc! { "sequence": -1 })
            .limit(1)
            .build();

        let mut cursor = self.events.find(filter).with_options(options).await?;

        if cursor.advance().await? {
            let doc = cursor.deserialize_current()?;
            Ok(doc.get_i32("sequence").unwrap_or(0) as u32 + 1)
        } else {
            Ok(0)
        }
    }

    async fn get_by_correlation(
        &self,
        _correlation_id: &str,
    ) -> Result<Vec<crate::proto::EventBook>> {
        // Not implemented for MongoDB - correlation_id not indexed
        Ok(vec![])
    }
}
