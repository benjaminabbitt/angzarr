//! MongoDB EventStore implementation.
//!
//! Implements composite reads for editions: query edition events first to derive
//! the implicit divergence point, then query main timeline up to that point,
//! then merge the results.

use async_trait::async_trait;
use mongodb::bson::{doc, Binary, Bson};
use mongodb::options::{FindOptions, IndexOptions};
use mongodb::{Client, Collection, Database, IndexModel};
use prost::Message;
use uuid::Uuid;

use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::storage::{EventStore, Result, StorageError};
use crate::proto::EventPage;

use super::EVENTS_COLLECTION;

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

        // Index for correlation ID queries
        let correlation_index = IndexModel::builder()
            .keys(doc! { "correlation_id": 1 })
            .build();

        self.events.create_index(correlation_index).await?;

        Ok(())
    }

    /// Get the database reference for transaction support.
    pub fn database(&self) -> &Database {
        &self.database
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
        root_str: &str,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        let filter = doc! {
            "edition": edition,
            "domain": domain,
            "root": root_str,
            "sequence": { "$gte": from as i32 }
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
                        root: Uuid::parse_str(root_str).unwrap_or_default(),
                    })?;
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    /// Get the minimum sequence number from edition events (implicit divergence point).
    async fn get_edition_min_sequence(
        &self,
        domain: &str,
        edition: &str,
        root_str: &str,
    ) -> Result<Option<u32>> {
        let filter = doc! {
            "edition": edition,
            "domain": domain,
            "root": root_str
        };

        let options = FindOptions::builder()
            .sort(doc! { "sequence": 1 })
            .limit(1)
            .build();

        let mut cursor = self.events.find(filter).with_options(options).await?;

        if cursor.advance().await? {
            let doc = cursor.deserialize_current()?;
            let min_seq = doc.get_i32("sequence").unwrap_or(0) as u32;
            Ok(Some(min_seq))
        } else {
            Ok(None)
        }
    }

    /// Query main timeline events in range [from, until).
    async fn query_main_events_range(
        &self,
        domain: &str,
        root_str: &str,
        from: u32,
        until_seq: u32,
    ) -> Result<Vec<EventPage>> {
        if from >= until_seq {
            return Ok(Vec::new());
        }

        let filter = doc! {
            "edition": DEFAULT_EDITION,
            "domain": domain,
            "root": root_str,
            "sequence": { "$gte": from as i32, "$lt": until_seq as i32 }
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
                        root: Uuid::parse_str(root_str).unwrap_or_default(),
                    })?;
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
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
        root_str: &str,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        // Get divergence point without fetching all edition events
        let divergence = match self.get_edition_min_sequence(domain, edition, root_str).await? {
            Some(d) => d,
            None => {
                // No edition events - return main timeline only
                return self.query_edition_events(domain, DEFAULT_EDITION, root_str, from).await;
            }
        };

        // Now fetch only the events we need:
        // - Main timeline: [from, divergence) if from < divergence
        // - Edition: [max(from, divergence), ∞)

        let mut result = Vec::new();

        // Main timeline events: only if from < divergence
        if from < divergence {
            let main_events = self.query_main_events_range(domain, root_str, from, divergence).await?;
            result.extend(main_events);
        }

        // Edition events: from max(from, divergence) onwards
        let edition_from = from.max(divergence);
        let edition_events = self.query_edition_events(domain, edition, root_str, edition_from).await?;
        result.extend(edition_events);

        Ok(result)
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
        correlation_id: &str,
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
                "correlation_id": correlation_id,
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

        // Main timeline: simple query
        if Self::is_main_timeline(edition) {
            tracing::info!(
                domain = domain,
                root = %root_str,
                from = from,
                collection = %self.events.name(),
                database = %self.database.name(),
                "MongoDB get_from query starting (main timeline)"
            );
            return self.query_edition_events(domain, DEFAULT_EDITION, &root_str, from).await;
        }

        // Named edition: composite read (main timeline up to divergence + edition events)
        tracing::info!(
            domain = domain,
            edition = edition,
            root = %root_str,
            from = from,
            collection = %self.events.name(),
            database = %self.database.name(),
            "MongoDB get_from query starting (composite read)"
        );
        self.composite_read(domain, edition, &root_str, from).await
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

        // For non-default editions with implicit divergence, we need composite logic:
        // If the edition has no events yet, use the main timeline's max sequence
        if !Self::is_main_timeline(edition) {
            let edition_filter = doc! { "edition": edition, "domain": domain, "root": &root_str };
            let options = FindOptions::builder()
                .sort(doc! { "sequence": -1 })
                .limit(1)
                .build();

            let mut cursor = self.events.find(edition_filter).with_options(options).await?;

            if cursor.advance().await? {
                let doc = cursor.deserialize_current()?;
                // Edition has events, use edition's max sequence
                return Ok(doc.get_i32("sequence").unwrap_or(0) as u32 + 1);
            }

            // No edition events - fall through to check main timeline
        }

        // Query the target edition (or main timeline for fallback)
        let target_edition = if Self::is_main_timeline(edition) {
            edition
        } else {
            DEFAULT_EDITION
        };

        let filter = doc! { "edition": target_edition, "domain": domain, "root": &root_str };
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
        correlation_id: &str,
    ) -> Result<Vec<crate::proto::EventBook>> {
        use crate::proto::{Cover, Edition, EventBook, Uuid as ProtoUuid};
        use std::collections::HashMap;

        if correlation_id.is_empty() {
            return Ok(vec![]);
        }

        let filter = doc! { "correlation_id": correlation_id };
        let options = FindOptions::builder()
            .sort(doc! { "domain": 1, "root": 1, "sequence": 1 })
            .build();

        let mut cursor = self.events.find(filter).with_options(options).await?;

        // Group events by (domain, edition, root)
        let mut books_map: HashMap<(String, String, Uuid), Vec<EventPage>> = HashMap::new();

        while cursor.advance().await? {
            let doc = cursor.deserialize_current()?;

            let domain = doc.get_str("domain").unwrap_or_default().to_string();
            let edition = doc.get_str("edition").unwrap_or_default().to_string();
            let root_str = doc.get_str("root").unwrap_or_default();
            let event_data = doc
                .get_binary_generic("event_data")
                .map_err(|_| StorageError::NotFound {
                    domain: domain.clone(),
                    root: Uuid::nil(),
                })?;

            let root = Uuid::parse_str(root_str)?;
            let event = EventPage::decode(event_data.as_slice())?;

            books_map
                .entry((domain, edition, root))
                .or_default()
                .push(event);
        }

        let books = books_map
            .into_iter()
            .map(|((domain, edition, root), pages)| EventBook {
                cover: Some(Cover {
                    domain,
                    root: Some(ProtoUuid {
                        value: root.as_bytes().to_vec(),
                    }),
                    correlation_id: correlation_id.to_string(),
                    edition: Some(Edition { name: edition, divergences: vec![] }),
                }),
                pages,
                snapshot: None,
            })
            .collect();

        Ok(books)
    }

    async fn delete_edition_events(&self, domain: &str, edition: &str) -> Result<u32> {
        let filter = doc! {
            "edition": edition,
            "domain": domain,
        };

        let result = self.events.delete_many(filter).await?;
        Ok(result.deleted_count as u32)
    }
}
