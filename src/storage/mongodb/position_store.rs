//! MongoDB PositionStore implementation.

use async_trait::async_trait;
use mongodb::bson::{doc, Binary};
use mongodb::options::{IndexOptions, UpdateOptions};
use mongodb::{Client, Collection, IndexModel};

use crate::storage::{PositionStore, Result};

use super::POSITIONS_COLLECTION;

/// MongoDB implementation of PositionStore.
pub struct MongoPositionStore {
    positions: Collection<mongodb::bson::Document>,
}

impl MongoPositionStore {
    /// Create a new MongoDB position store.
    pub async fn new(client: &Client, database_name: &str) -> Result<Self> {
        let database = client.database(database_name);
        let positions = database.collection(POSITIONS_COLLECTION);

        let store = Self { positions };
        store.init().await?;

        Ok(store)
    }

    /// Initialize indexes.
    async fn init(&self) -> Result<()> {
        // Compound unique index on (handler, domain, root)
        let index = IndexModel::builder()
            .keys(doc! { "handler": 1, "edition": 1, "domain": 1, "root": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        self.positions.create_index(index).await?;

        Ok(())
    }
}

#[async_trait]
impl PositionStore for MongoPositionStore {
    async fn get(&self, handler: &str, domain: &str, edition: &str, root: &[u8]) -> Result<Option<u32>> {
        let root_binary = Binary {
            subtype: mongodb::bson::spec::BinarySubtype::Generic,
            bytes: root.to_vec(),
        };

        let filter = doc! {
            "handler": handler,
            "edition": edition,
            "domain": domain,
            "root": &root_binary,
        };

        let result = self.positions.find_one(filter).await?;

        match result {
            Some(doc) => {
                let sequence = doc.get_i32("sequence").unwrap_or(0) as u32;
                Ok(Some(sequence))
            }
            None => Ok(None),
        }
    }

    async fn put(&self, handler: &str, domain: &str, edition: &str, root: &[u8], sequence: u32) -> Result<()> {
        let root_binary = Binary {
            subtype: mongodb::bson::spec::BinarySubtype::Generic,
            bytes: root.to_vec(),
        };
        let updated_at = chrono::Utc::now().to_rfc3339();

        let filter = doc! {
            "handler": handler,
            "edition": edition,
            "domain": domain,
            "root": &root_binary,
        };

        let update = doc! {
            "$set": {
                "handler": handler,
                "edition": edition,
                "domain": domain,
                "root": &root_binary,
                "sequence": sequence as i32,
                "updated_at": &updated_at,
            }
        };

        let options = UpdateOptions::builder().upsert(true).build();

        self.positions
            .update_one(filter, update)
            .with_options(options)
            .await?;

        Ok(())
    }
}
