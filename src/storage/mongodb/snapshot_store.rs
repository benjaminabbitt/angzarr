//! MongoDB SnapshotStore implementation.

use async_trait::async_trait;
use mongodb::bson::{doc, Binary};
use mongodb::options::{IndexOptions, UpdateOptions};
use mongodb::{Client, Collection, IndexModel};
use prost::Message;
use uuid::Uuid;

use crate::proto::Snapshot;
use crate::storage::{Result, SnapshotStore, StorageError};

use super::SNAPSHOTS_COLLECTION;

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
            .keys(doc! { "edition": 1, "domain": 1, "root": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        self.snapshots.create_index(index).await?;

        Ok(())
    }
}

#[async_trait]
impl SnapshotStore for MongoSnapshotStore {
    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let root_str = root.to_string();

        let filter = doc! { "edition": edition, "domain": domain, "root": &root_str };

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

    async fn put(&self, domain: &str, edition: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let root_str = root.to_string();
        let state_data = snapshot.encode_to_vec();
        let created_at = chrono::Utc::now().to_rfc3339();

        let filter = doc! { "edition": edition, "domain": domain, "root": &root_str };

        let update = doc! {
            "$set": {
                "edition": edition,
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

    async fn delete(&self, domain: &str, edition: &str, root: Uuid) -> Result<()> {
        let root_str = root.to_string();

        let filter = doc! { "edition": edition, "domain": domain, "root": &root_str };

        self.snapshots.delete_one(filter).await?;

        Ok(())
    }
}
