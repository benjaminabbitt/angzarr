//! Mock PositionStore for testing.

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;

use crate::storage::{PositionStore, Result};

/// Mock position store for testing.
///
/// Uses an in-memory HashMap to track handler checkpoints.
/// Thread-safe via RwLock for use in async tests.
pub struct MockPositionStore {
    positions: RwLock<HashMap<String, u32>>,
}

impl MockPositionStore {
    /// Create a new mock position store.
    pub fn new() -> Self {
        Self {
            positions: RwLock::new(HashMap::new()),
        }
    }

    /// Create a key from handler/domain/edition/root.
    fn make_key(handler: &str, domain: &str, edition: &str, root: &[u8]) -> String {
        format!("{}:{}:{}:{}", handler, domain, edition, hex::encode(root))
    }
}

impl Default for MockPositionStore {
    fn default() -> Self {
        Self::new()
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
        let key = Self::make_key(handler, domain, edition, root);
        Ok(self.positions.read().unwrap().get(&key).copied())
    }

    async fn put(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
        sequence: u32,
    ) -> Result<()> {
        let key = Self::make_key(handler, domain, edition, root);
        self.positions.write().unwrap().insert(key, sequence);
        Ok(())
    }
}
