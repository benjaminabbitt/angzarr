//! Mock PositionStore implementation for testing.

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::storage::{PositionStore, Result};

type PositionKey = (String, String, String, Vec<u8>);

/// Mock position store that stores positions in memory.
#[derive(Default)]
pub struct MockPositionStore {
    positions: RwLock<HashMap<PositionKey, u32>>,
}

impl MockPositionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PositionStore for MockPositionStore {
    async fn get(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
    ) -> Result<Option<u32>> {
        let key = (
            handler.to_string(),
            domain.to_string(),
            edition.to_string(),
            root.to_vec(),
        );
        Ok(self.positions.read().await.get(&key).copied())
    }

    async fn put(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
        sequence: u32,
    ) -> Result<()> {
        let key = (
            handler.to_string(),
            domain.to_string(),
            edition.to_string(),
            root.to_vec(),
        );
        self.positions.write().await.insert(key, sequence);
        Ok(())
    }
}
