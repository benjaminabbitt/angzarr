//! DynamoDB PositionStore implementation (placeholder).

use async_trait::async_trait;

use crate::storage::{PositionStore, Result};

/// DynamoDB implementation of PositionStore.
pub struct DynamoPositionStore;

#[async_trait]
impl PositionStore for DynamoPositionStore {
    async fn get(&self, _handler: &str, _domain: &str, _root: &[u8]) -> Result<Option<u32>> {
        todo!("DynamoPositionStore::get")
    }

    async fn put(&self, _handler: &str, _domain: &str, _root: &[u8], _sequence: u32) -> Result<()> {
        todo!("DynamoPositionStore::put")
    }
}
