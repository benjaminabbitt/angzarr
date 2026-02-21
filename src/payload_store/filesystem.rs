//! Filesystem-based payload storage.
//!
//! Stores payloads as files in a directory structure:
//! ```text
//! {base_path}/
//!   {hash[0:2]}/
//!     {hash}.bin
//! ```
//!
//! The first two characters of the hash create a subdirectory to avoid
//! having too many files in a single directory.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use prost_types::Timestamp;
use tokio::fs;
use tracing::{debug, warn};

use super::{compute_hash, hash_to_hex, PayloadStore, PayloadStoreError, Result};
use crate::proto::{PayloadReference, PayloadStorageType};

/// Filesystem-based payload store.
///
/// Stores payloads in content-addressable files under a base directory.
pub struct FilesystemPayloadStore {
    base_path: PathBuf,
}

impl FilesystemPayloadStore {
    /// Create a new filesystem payload store.
    ///
    /// Creates the base directory if it doesn't exist.
    pub async fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path).await?;
        Ok(Self { base_path })
    }

    /// Get the file path for a given hash.
    fn path_for_hash(&self, hash: &[u8]) -> PathBuf {
        let hex = hash_to_hex(hash);
        // Use first 2 chars as subdirectory to avoid too many files in one dir
        let subdir = &hex[0..2];
        self.base_path.join(subdir).join(format!("{}.bin", hex))
    }

    /// Build a URI for a file path.
    fn uri_for_path(&self, path: &Path) -> String {
        format!("file://{}", path.display())
    }

    /// Extract path from a file URI.
    fn path_from_uri(&self, uri: &str) -> Result<PathBuf> {
        uri.strip_prefix("file://")
            .map(PathBuf::from)
            .ok_or_else(|| PayloadStoreError::InvalidUri(format!("Not a file URI: {}", uri)))
    }
}

#[async_trait]
impl PayloadStore for FilesystemPayloadStore {
    async fn put(&self, payload: &[u8]) -> Result<PayloadReference> {
        let hash = compute_hash(payload);
        let path = self.path_for_hash(&hash);

        // Check if already exists (deduplication)
        if path.exists() {
            debug!(
                hash = %hash_to_hex(&hash),
                "Payload already exists, returning existing reference"
            );
        } else {
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }

            // Write atomically using temp file + rename
            let temp_path = path.with_extension("tmp");
            fs::write(&temp_path, payload).await?;
            fs::rename(&temp_path, &path).await?;

            debug!(
                hash = %hash_to_hex(&hash),
                size = payload.len(),
                "Stored payload"
            );
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();

        Ok(PayloadReference {
            storage_type: PayloadStorageType::Filesystem as i32,
            uri: self.uri_for_path(&path),
            content_hash: hash,
            original_size: payload.len() as u64,
            stored_at: Some(Timestamp {
                seconds: now.as_secs() as i64,
                nanos: now.subsec_nanos() as i32,
            }),
        })
    }

    async fn get(&self, reference: &PayloadReference) -> Result<Vec<u8>> {
        let path = self.path_from_uri(&reference.uri)?;

        if !path.exists() {
            return Err(PayloadStoreError::NotFound(reference.uri.clone()));
        }

        let payload = fs::read(&path).await?;

        // Verify integrity
        let actual_hash = compute_hash(&payload);
        if actual_hash != reference.content_hash {
            return Err(PayloadStoreError::IntegrityFailed {
                expected: hash_to_hex(&reference.content_hash),
                actual: hash_to_hex(&actual_hash),
            });
        }

        Ok(payload)
    }

    async fn delete_older_than(&self, age: Duration) -> Result<usize> {
        let cutoff = SystemTime::now() - age;
        let mut deleted = 0;

        // Walk the base directory
        let mut entries = fs::read_dir(&self.base_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Skip if not a directory (subdirectories)
            if !path.is_dir() {
                continue;
            }

            // Walk subdirectory
            let mut subentries = fs::read_dir(&path).await?;
            while let Some(subentry) = subentries.next_entry().await? {
                let file_path = subentry.path();

                // Skip non-.bin files
                if file_path.extension().is_none_or(|e| e != "bin") {
                    continue;
                }

                // Check modification time
                let metadata = fs::metadata(&file_path).await?;
                if let Ok(modified) = metadata.modified() {
                    if modified < cutoff {
                        if let Err(e) = fs::remove_file(&file_path).await {
                            warn!(
                                path = %file_path.display(),
                                error = %e,
                                "Failed to delete expired payload"
                            );
                        } else {
                            deleted += 1;
                        }
                    }
                }
            }
        }

        Ok(deleted)
    }

    fn storage_type(&self) -> PayloadStorageType {
        PayloadStorageType::Filesystem
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_temp_store() -> (FilesystemPayloadStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = FilesystemPayloadStore::new(temp_dir.path()).await.unwrap();
        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_put_and_get() {
        let (store, _temp) = create_temp_store().await;
        let payload = b"test payload data";

        let reference = store.put(payload).await.unwrap();

        assert_eq!(
            reference.storage_type,
            PayloadStorageType::Filesystem as i32
        );
        assert!(reference.uri.starts_with("file://"));
        assert_eq!(reference.original_size, payload.len() as u64);
        assert!(!reference.content_hash.is_empty());

        let retrieved = store.get(&reference).await.unwrap();
        assert_eq!(retrieved, payload);
    }

    #[tokio::test]
    async fn test_deduplication() {
        let (store, _temp) = create_temp_store().await;
        let payload = b"duplicate payload";

        let ref1 = store.put(payload).await.unwrap();
        let ref2 = store.put(payload).await.unwrap();

        // Same hash means same URI
        assert_eq!(ref1.uri, ref2.uri);
        assert_eq!(ref1.content_hash, ref2.content_hash);
    }

    #[tokio::test]
    async fn test_different_payloads() {
        let (store, _temp) = create_temp_store().await;

        let ref1 = store.put(b"payload one").await.unwrap();
        let ref2 = store.put(b"payload two").await.unwrap();

        assert_ne!(ref1.uri, ref2.uri);
        assert_ne!(ref1.content_hash, ref2.content_hash);
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let (store, _temp) = create_temp_store().await;

        let reference = PayloadReference {
            storage_type: PayloadStorageType::Filesystem as i32,
            uri: "file:///nonexistent/path/hash.bin".to_string(),
            content_hash: vec![0; 32],
            original_size: 100,
            stored_at: None,
        };

        let result = store.get(&reference).await;
        assert!(matches!(result, Err(PayloadStoreError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_integrity_check() {
        let (store, _temp) = create_temp_store().await;
        let payload = b"original payload";

        let mut reference = store.put(payload).await.unwrap();

        // Corrupt the hash
        reference.content_hash[0] ^= 0xFF;

        let result = store.get(&reference).await;
        assert!(matches!(
            result,
            Err(PayloadStoreError::IntegrityFailed { .. })
        ));
    }

    #[tokio::test]
    async fn test_delete_older_than() {
        let (store, _temp) = create_temp_store().await;

        // Store a payload
        let reference = store.put(b"old payload").await.unwrap();
        let path = store.path_from_uri(&reference.uri).unwrap();

        // Verify it exists
        assert!(path.exists());

        // Delete nothing (age = 1 hour, file is new)
        let deleted = store
            .delete_older_than(Duration::from_secs(3600))
            .await
            .unwrap();
        assert_eq!(deleted, 0);
        assert!(path.exists());

        // Manually backdate the file
        let old_time =
            std::fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(7200));
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(old_time)
            .unwrap();

        // Now delete (age = 1 hour, file is 2 hours old)
        let deleted = store
            .delete_older_than(Duration::from_secs(3600))
            .await
            .unwrap();
        assert_eq!(deleted, 1);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_invalid_uri() {
        let (store, _temp) = create_temp_store().await;

        let reference = PayloadReference {
            storage_type: PayloadStorageType::Filesystem as i32,
            uri: "gs://bucket/key".to_string(), // Wrong scheme
            content_hash: vec![0; 32],
            original_size: 100,
            stored_at: None,
        };

        let result = store.get(&reference).await;
        assert!(matches!(result, Err(PayloadStoreError::InvalidUri(_))));
    }
}
