//! Redis PositionStore implementation (placeholder).

use async_trait::async_trait;

use crate::storage::{PositionStore, Result};

/// Redis implementation of PositionStore.
pub struct RedisPositionStore;

#[async_trait]
impl PositionStore for RedisPositionStore {
    async fn get(&self, _handler: &str, _domain: &str, _edition: &str, _root: &[u8]) -> Result<Option<u32>> {
        todo!("RedisPositionStore::get")
    }

    async fn put(&self, _handler: &str, _domain: &str, _edition: &str, _root: &[u8], _sequence: u32) -> Result<()> {
        todo!("RedisPositionStore::put")
    }
}
