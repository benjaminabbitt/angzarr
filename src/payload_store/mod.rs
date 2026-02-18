//! External payload storage for claim check pattern.
//!
//! DOC: This file is referenced in docs/docs/operations/payload-offloading.md
//!      Update documentation when making changes to payload store patterns.
//!
//! When event/command payloads exceed message bus size limits, they are stored
//! externally and replaced with a `PayloadReference` marker. This module provides
//! the storage backends and related infrastructure.
//!
//! ## Storage Backends
//!
//! - `FilesystemPayloadStore` - Local filesystem storage
//! - `GcsPayloadStore` (feature: gcs) - Google Cloud Storage
//! - `S3PayloadStore` (feature: s3) - Amazon S3
//!
//! ## Content-Addressable Storage
//!
//! All backends use SHA-256 content hashing for:
//! - Deduplication: identical payloads share storage
//! - Integrity verification: corruption detection on retrieval
//!
//! ## TTL Cleanup
//!
//! Use `TtlReaper` to periodically clean up expired payloads.

mod config;
mod filesystem;
#[cfg(feature = "gcs")]
mod gcs;
mod reaper;
#[cfg(feature = "s3")]
mod s3;

#[cfg(feature = "gcs")]
pub use config::GcsStoreConfig;
#[cfg(feature = "s3")]
pub use config::S3StoreConfig;
pub use config::{FilesystemStoreConfig, PayloadOffloadConfig, PayloadStoreType};
pub use filesystem::FilesystemPayloadStore;
#[cfg(feature = "gcs")]
pub use gcs::GcsPayloadStore;
pub use reaper::TtlReaper;
#[cfg(feature = "s3")]
pub use s3::S3PayloadStore;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::proto::{PayloadReference, PayloadStorageType};

/// Errors that can occur during payload store operations.
#[derive(Debug, Error)]
pub enum PayloadStoreError {
    #[error("Failed to store payload: {0}")]
    StoreFailed(String),

    #[error("Failed to retrieve payload: {0}")]
    RetrieveFailed(String),

    #[error("Payload not found: {0}")]
    NotFound(String),

    #[error("Integrity check failed: expected {expected}, got {actual}")]
    IntegrityFailed { expected: String, actual: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid URI: {0}")]
    InvalidUri(String),
}

/// Result type for payload store operations.
pub type Result<T> = std::result::Result<T, PayloadStoreError>;

/// External payload storage backend for claim check pattern.
///
/// Implementations store large payloads externally and return references
/// that can be embedded in EventPage/CommandPage messages.
#[async_trait]
pub trait PayloadStore: Send + Sync {
    /// Store payload and return a reference.
    ///
    /// Uses content-addressable storage (hash-based naming) for deduplication.
    /// If a payload with the same hash already exists, returns the existing reference.
    async fn put(&self, payload: &[u8]) -> Result<PayloadReference>;

    /// Retrieve payload by reference.
    ///
    /// Verifies content hash matches the reference for integrity.
    async fn get(&self, reference: &PayloadReference) -> Result<Vec<u8>>;

    /// Delete payloads older than the given duration.
    ///
    /// Called by TTL reaper background task.
    /// Returns the number of payloads deleted.
    async fn delete_older_than(&self, age: Duration) -> Result<usize>;

    /// Storage type for this backend.
    fn storage_type(&self) -> PayloadStorageType;
}

/// Compute SHA-256 hash of payload.
pub fn compute_hash(payload: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    hasher.finalize().to_vec()
}

/// Format hash as hex string for filenames.
pub fn hash_to_hex(hash: &[u8]) -> String {
    hex::encode(hash)
}

/// Parse hex string back to hash bytes.
pub fn hex_to_hash(hex_str: &str) -> Result<Vec<u8>> {
    hex::decode(hex_str).map_err(|e| PayloadStoreError::InvalidUri(e.to_string()))
}

// ============================================================================
// Factory
// ============================================================================

/// Initialize a payload store based on configuration.
///
/// Returns `None` if payload offloading is disabled.
///
/// # Errors
///
/// Returns error if the configured store type requires an unavailable feature.
pub async fn init_payload_store(
    config: &PayloadOffloadConfig,
) -> std::result::Result<Option<Arc<dyn PayloadStore>>, Box<dyn std::error::Error>> {
    use tracing::info;

    if !config.enabled {
        return Ok(None);
    }

    match config.store_type {
        PayloadStoreType::Filesystem => {
            info!(
                path = %config.filesystem.base_path.display(),
                "PayloadStore: filesystem"
            );
            let store = FilesystemPayloadStore::new(&config.filesystem.base_path).await?;
            Ok(Some(Arc::new(store)))
        }
        #[cfg(feature = "gcs")]
        PayloadStoreType::Gcs => {
            info!(
                bucket = %config.gcs.bucket,
                prefix = ?config.gcs.prefix,
                "PayloadStore: gcs"
            );
            let store = GcsPayloadStore::new(&config.gcs.bucket, config.gcs.prefix.clone()).await?;
            Ok(Some(Arc::new(store)))
        }
        #[cfg(feature = "s3")]
        PayloadStoreType::S3 => {
            info!(
                bucket = %config.s3.bucket,
                prefix = ?config.s3.prefix,
                region = ?config.s3.region,
                endpoint = ?config.s3.endpoint,
                "PayloadStore: s3"
            );
            let store = match &config.s3.endpoint {
                Some(endpoint) => {
                    S3PayloadStore::with_endpoint(
                        &config.s3.bucket,
                        config.s3.prefix.clone(),
                        endpoint,
                        config.s3.region.as_deref(),
                    )
                    .await?
                }
                None => S3PayloadStore::new(&config.s3.bucket, config.s3.prefix.clone()).await?,
            };
            Ok(Some(Arc::new(store)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_compute_hash_deterministic() {
        let payload = b"test payload data";
        let hash1 = compute_hash(payload);
        let hash2 = compute_hash(payload);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 32); // SHA-256 produces 32 bytes
    }

    #[test]
    fn test_compute_hash_different_inputs() {
        let hash1 = compute_hash(b"payload 1");
        let hash2 = compute_hash(b"payload 2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_hex_roundtrip() {
        let original = compute_hash(b"test");
        let hex = hash_to_hex(&original);
        let recovered = hex_to_hash(&hex).unwrap();
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_hex_to_hash_invalid() {
        let result = hex_to_hash("not valid hex!");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_init_payload_store_disabled() {
        let config = PayloadOffloadConfig {
            enabled: false,
            ..Default::default()
        };

        let result = init_payload_store(&config).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_init_payload_store_filesystem() {
        let temp_dir = TempDir::new().unwrap();
        let config = PayloadOffloadConfig {
            enabled: true,
            store_type: PayloadStoreType::Filesystem,
            filesystem: FilesystemStoreConfig {
                base_path: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let result = init_payload_store(&config).await.unwrap();
        assert!(result.is_some());

        let store = result.unwrap();
        assert_eq!(
            store.storage_type(),
            crate::proto::PayloadStorageType::Filesystem
        );
    }

    #[tokio::test]
    async fn test_init_payload_store_can_store_and_retrieve() {
        let temp_dir = TempDir::new().unwrap();
        let config = PayloadOffloadConfig {
            enabled: true,
            store_type: PayloadStoreType::Filesystem,
            filesystem: FilesystemStoreConfig {
                base_path: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let store = init_payload_store(&config).await.unwrap().unwrap();

        let payload = b"test payload from factory";
        let reference = store.put(payload).await.unwrap();
        let retrieved = store.get(&reference).await.unwrap();

        assert_eq!(payload.as_slice(), retrieved.as_slice());
    }
}
