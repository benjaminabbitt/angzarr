//! Bigtable PositionStore implementation (placeholder).

use async_trait::async_trait;

use crate::storage::{PositionStore, Result};

/// Bigtable implementation of PositionStore.
pub struct BigtablePositionStore;

#[async_trait]
impl PositionStore for BigtablePositionStore {
    async fn get(&self, _handler: &str, _domain: &str, _root: &[u8]) -> Result<Option<u32>> {
        todo!("BigtablePositionStore::get")
    }

    async fn put(&self, _handler: &str, _domain: &str, _root: &[u8], _sequence: u32) -> Result<()> {
        todo!("BigtablePositionStore::put")
    }
}
