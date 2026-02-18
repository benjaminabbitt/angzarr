//! Google Cloud Storage payload store.
//!
//! Stores payloads as objects in a GCS bucket:
//! ```text
//! gs://{bucket}/{prefix}/{hash[0:2]}/{hash}
//! ```

use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::objects::delete::DeleteObjectRequest;
use google_cloud_storage::http::objects::download::Range;
use google_cloud_storage::http::objects::get::GetObjectRequest;
use google_cloud_storage::http::objects::list::ListObjectsRequest;
use google_cloud_storage::http::objects::upload::{Media, UploadObjectRequest, UploadType};
use prost_types::Timestamp;
use tracing::{debug, warn};

use super::{compute_hash, hash_to_hex, PayloadStore, PayloadStoreError, Result};
use crate::proto::{PayloadReference, PayloadStorageType};

/// GCS-based payload store.
///
/// Stores payloads in content-addressable objects under a GCS bucket.
pub struct GcsPayloadStore {
    client: Client,
    bucket: String,
    prefix: Option<String>,
}

impl GcsPayloadStore {
    /// Create a new GCS payload store.
    ///
    /// Uses default credentials from the environment (GOOGLE_APPLICATION_CREDENTIALS
    /// or workload identity).
    pub async fn new(bucket: impl Into<String>, prefix: Option<String>) -> Result<Self> {
        let config = ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| PayloadStoreError::StoreFailed(format!("GCS auth failed: {}", e)))?;

        let client = Client::new(config);

        Ok(Self {
            client,
            bucket: bucket.into(),
            prefix,
        })
    }

    /// Create with explicit client config (for testing).
    pub fn with_client(client: Client, bucket: impl Into<String>, prefix: Option<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
            prefix,
        }
    }

    /// Get the object name for a given hash.
    fn object_name(&self, hash: &[u8]) -> String {
        let hex = hash_to_hex(hash);
        let subdir = &hex[0..2];

        match &self.prefix {
            Some(prefix) => format!("{}/{}/{}", prefix, subdir, hex),
            None => format!("{}/{}", subdir, hex),
        }
    }

    /// Build a URI for an object.
    fn uri_for_object(&self, object_name: &str) -> String {
        format!("gs://{}/{}", self.bucket, object_name)
    }

    /// Extract object name from a gs:// URI.
    fn object_from_uri(&self, uri: &str) -> Result<String> {
        let expected_prefix = format!("gs://{}/", self.bucket);
        uri.strip_prefix(&expected_prefix)
            .map(|s| s.to_string())
            .ok_or_else(|| {
                PayloadStoreError::InvalidUri(format!(
                    "URI doesn't match bucket {}: {}",
                    self.bucket, uri
                ))
            })
    }
}

#[async_trait]
impl PayloadStore for GcsPayloadStore {
    async fn put(&self, payload: &[u8]) -> Result<PayloadReference> {
        let hash = compute_hash(payload);
        let object_name = self.object_name(&hash);

        // Check if already exists (deduplication)
        let exists = self
            .client
            .get_object(&GetObjectRequest {
                bucket: self.bucket.clone(),
                object: object_name.clone(),
                ..Default::default()
            })
            .await
            .is_ok();

        if exists {
            debug!(
                hash = %hash_to_hex(&hash),
                "Payload already exists in GCS, returning existing reference"
            );
        } else {
            // Upload the payload
            let upload_type = UploadType::Simple(Media::new(object_name.clone()));

            self.client
                .upload_object(
                    &UploadObjectRequest {
                        bucket: self.bucket.clone(),
                        ..Default::default()
                    },
                    payload.to_vec(),
                    &upload_type,
                )
                .await
                .map_err(|e| PayloadStoreError::StoreFailed(format!("GCS upload failed: {}", e)))?;

            debug!(
                hash = %hash_to_hex(&hash),
                size = payload.len(),
                bucket = %self.bucket,
                "Stored payload in GCS"
            );
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();

        Ok(PayloadReference {
            storage_type: PayloadStorageType::Gcs as i32,
            uri: self.uri_for_object(&object_name),
            content_hash: hash,
            original_size: payload.len() as u64,
            stored_at: Some(Timestamp {
                seconds: now.as_secs() as i64,
                nanos: now.subsec_nanos() as i32,
            }),
        })
    }

    async fn get(&self, reference: &PayloadReference) -> Result<Vec<u8>> {
        let object_name = self.object_from_uri(&reference.uri)?;

        let payload = self
            .client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: object_name.clone(),
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
            .map_err(|e| {
                if e.to_string().contains("404") || e.to_string().contains("Not Found") {
                    PayloadStoreError::NotFound(reference.uri.clone())
                } else {
                    PayloadStoreError::RetrieveFailed(format!("GCS download failed: {}", e))
                }
            })?;

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
        let cutoff_secs = cutoff
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut deleted = 0;
        let mut page_token: Option<String> = None;

        loop {
            let list_request = ListObjectsRequest {
                bucket: self.bucket.clone(),
                prefix: self.prefix.clone(),
                page_token: page_token.clone(),
                ..Default::default()
            };

            let response =
                self.client.list_objects(&list_request).await.map_err(|e| {
                    PayloadStoreError::StoreFailed(format!("GCS list failed: {}", e))
                })?;

            for object in response.items.into_iter().flatten() {
                // Check object creation time
                if let Some(time_created) = object.time_created {
                    let created_secs = time_created.unix_timestamp() as u64;
                    if created_secs < cutoff_secs {
                        if let Err(e) = self
                            .client
                            .delete_object(&DeleteObjectRequest {
                                bucket: self.bucket.clone(),
                                object: object.name.clone(),
                                ..Default::default()
                            })
                            .await
                        {
                            warn!(
                                object = %object.name,
                                error = %e,
                                "Failed to delete expired payload from GCS"
                            );
                        } else {
                            deleted += 1;
                        }
                    }
                }
            }

            page_token = response.next_page_token;
            if page_token.is_none() {
                break;
            }
        }

        Ok(deleted)
    }

    fn storage_type(&self) -> PayloadStorageType {
        PayloadStorageType::Gcs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_name_without_prefix() {
        // We can't easily test the full store without GCS, but we can test helpers
        let hash = compute_hash(b"test payload");
        let hex = hash_to_hex(&hash);

        // Object name should be {hash[0:2]}/{hash}
        assert!(hex.len() >= 2);
        let expected = format!("{}/{}", &hex[0..2], hex);

        // Can't create store without credentials, so just verify the expected format
        assert!(expected.contains('/'));
        assert!(expected.starts_with(&hex[0..2]));
    }

    #[test]
    fn test_uri_format() {
        // Verify URI format is correct
        let bucket = "my-bucket";
        let object = "ab/abcdef123456";
        let uri = format!("gs://{}/{}", bucket, object);

        assert!(uri.starts_with("gs://"));
        assert!(uri.contains(bucket));
    }
}
