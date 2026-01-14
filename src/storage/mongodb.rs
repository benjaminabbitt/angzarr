//! MongoDB implementations of storage interfaces.

use async_trait::async_trait;
use mongodb::bson::{doc, Binary, Bson};
use mongodb::options::{FindOptions, IndexOptions, UpdateOptions};
use mongodb::{Client, Collection, Database, IndexModel};
use prost::Message;
use uuid::Uuid;

use crate::interfaces::event_store::{EventStore, Result, StorageError};
use crate::interfaces::snapshot_store::SnapshotStore;
use crate::proto::{EventPage, Snapshot};

/// Collection names.
const EVENTS_COLLECTION: &str = "events";
const SNAPSHOTS_COLLECTION: &str = "snapshots";

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
            .keys(doc! { "domain": 1, "root": 1, "sequence": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        self.events.create_index(index).await?;

        // Index for listing roots by domain
        let domain_index = IndexModel::builder().keys(doc! { "domain": 1 }).build();

        self.events.create_index(domain_index).await?;

        Ok(())
    }

    /// Get the database reference for transaction support.
    pub fn database(&self) -> &Database {
        &self.database
    }
}

#[async_trait]
impl EventStore for MongoEventStore {
    async fn add(&self, domain: &str, root: Uuid, events: Vec<EventPage>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let root_str = root.to_string();

        // Start a session for transaction support
        let mut session = self.database.client().start_session().await?;
        session.start_transaction().await?;

        // Get the next sequence number within the transaction
        let base_sequence = {
            let filter = doc! { "domain": domain, "root": &root_str };
            let options = FindOptions::builder()
                .sort(doc! { "sequence": -1 })
                .limit(1)
                .build();

            let mut cursor = self
                .events
                .find(filter)
                .with_options(options)
                .session(&mut session)
                .await?;

            if cursor.advance(&mut session).await? {
                let doc = cursor.deserialize_current()?;
                doc.get_i32("sequence").unwrap_or(0) as u32 + 1
            } else {
                0
            }
        };

        let mut auto_sequence = base_sequence;

        for event in events {
            let event_data = event.encode_to_vec();

            // Determine sequence number
            let sequence = match &event.sequence {
                Some(crate::proto::event_page::Sequence::Num(n)) => {
                    if *n < base_sequence {
                        session.abort_transaction().await?;
                        return Err(StorageError::SequenceConflict {
                            expected: base_sequence,
                            actual: *n,
                        });
                    }
                    *n
                }
                Some(crate::proto::event_page::Sequence::Force(_)) | None => {
                    let seq = auto_sequence;
                    auto_sequence += 1;
                    seq
                }
            };

            let created_at = event
                .created_at
                .as_ref()
                .map(|ts| {
                    chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32).ok_or_else(|| {
                        StorageError::InvalidTimestamp {
                            seconds: ts.seconds,
                            nanos: ts.nanos,
                        }
                    })
                })
                .transpose()?
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

            let doc = doc! {
                "domain": domain,
                "root": &root_str,
                "sequence": sequence as i32,
                "created_at": &created_at,
                "event_data": Binary { subtype: mongodb::bson::spec::BinarySubtype::Generic, bytes: event_data },
                "synchronous": event.synchronous,
            };

            self.events.insert_one(doc).session(&mut session).await?;
        }

        session.commit_transaction().await?;

        Ok(())
    }

    async fn get(&self, domain: &str, root: Uuid) -> Result<Vec<EventPage>> {
        self.get_from(domain, root, 0).await
    }

    async fn get_from(&self, domain: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>> {
        let root_str = root.to_string();

        let filter = doc! {
            "domain": domain,
            "root": &root_str,
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
                        root,
                    })?;
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
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
        let root_str = root.to_string();

        let filter = doc! {
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

    async fn list_roots(&self, domain: &str) -> Result<Vec<Uuid>> {
        let pipeline = vec![
            doc! { "$match": { "domain": domain } },
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

    async fn get_next_sequence(&self, domain: &str, root: Uuid) -> Result<u32> {
        let root_str = root.to_string();

        let filter = doc! { "domain": domain, "root": &root_str };
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
}

/// MongoDB implementation of SnapshotStore.
pub struct MongoSnapshotStore {
    snapshots: Collection<mongodb::bson::Document>,
}

impl MongoSnapshotStore {
    /// Create a new MongoDB snapshot store.
    pub async fn new(client: &Client, database_name: &str) -> Result<Self> {
        let database = client.database(database_name);
        let snapshots = database.collection(SNAPSHOTS_COLLECTION);

        let store = Self { snapshots };
        store.init().await?;

        Ok(store)
    }

    /// Initialize indexes.
    async fn init(&self) -> Result<()> {
        // Unique index on (domain, root) - only one snapshot per aggregate
        let index = IndexModel::builder()
            .keys(doc! { "domain": 1, "root": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        self.snapshots.create_index(index).await?;

        Ok(())
    }
}

#[async_trait]
impl SnapshotStore for MongoSnapshotStore {
    async fn get(&self, domain: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let root_str = root.to_string();

        let filter = doc! { "domain": domain, "root": &root_str };

        let result = self.snapshots.find_one(filter).await?;

        match result {
            Some(doc) => {
                let state_data =
                    doc.get_binary_generic("state_data")
                        .map_err(|_| StorageError::NotFound {
                            domain: domain.to_string(),
                            root,
                        })?;
                let snapshot = Snapshot::decode(state_data.as_slice())?;
                Ok(Some(snapshot))
            }
            None => Ok(None),
        }
    }

    async fn put(&self, domain: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let root_str = root.to_string();
        let state_data = snapshot.encode_to_vec();
        let created_at = chrono::Utc::now().to_rfc3339();

        let filter = doc! { "domain": domain, "root": &root_str };

        let update = doc! {
            "$set": {
                "domain": domain,
                "root": &root_str,
                "sequence": snapshot.sequence as i32,
                "state_data": Binary { subtype: mongodb::bson::spec::BinarySubtype::Generic, bytes: state_data },
                "created_at": &created_at,
            }
        };

        let options = UpdateOptions::builder().upsert(true).build();

        self.snapshots
            .update_one(filter, update)
            .with_options(options)
            .await?;

        Ok(())
    }

    async fn delete(&self, domain: &str, root: Uuid) -> Result<()> {
        let root_str = root.to_string();

        let filter = doc! { "domain": domain, "root": &root_str };

        self.snapshots.delete_one(filter).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_names() {
        assert_eq!(EVENTS_COLLECTION, "events");
        assert_eq!(SNAPSHOTS_COLLECTION, "snapshots");
    }
}
