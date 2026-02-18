//! Amazon S3 payload store.
//!
//! Stores payloads as objects in an S3 bucket:
//! ```text
//! s3://{bucket}/{prefix}/{hash[0:2]}/{hash}
//! ```

use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use prost_types::Timestamp;
use tracing::{debug, warn};

use super::{compute_hash, hash_to_hex, PayloadStore, PayloadStoreError, Result};
use crate::proto::{PayloadReference, PayloadStorageType};

/// S3-based payload store.
///
/// Stores payloads in content-addressable objects under an S3 bucket.
pub struct S3PayloadStore {
    client: Client,
    bucket: String,
    prefix: Option<String>,
}

impl S3PayloadStore {
    /// Create a new S3 payload store.
    ///
    /// Uses default credentials from the environment (AWS_ACCESS_KEY_ID,
    /// AWS_SECRET_ACCESS_KEY, or IAM role).
    pub async fn new(bucket: impl Into<String>, prefix: Option<String>) -> Result<Self> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        Ok(Self {
            client,
            bucket: bucket.into(),
            prefix,
        })
    }

    /// Create with custom endpoint (for S3-compatible services like MinIO).
    pub async fn with_endpoint(
        bucket: impl Into<String>,
        prefix: Option<String>,
        endpoint: &str,
        region: Option<&str>,
    ) -> Result<Self> {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());

        if let Some(region) = region {
            config_loader = config_loader.region(aws_config::Region::new(region.to_string()));
        }

        let config = config_loader.load().await;

        let s3_config = aws_sdk_s3::config::Builder::from(&config)
            .endpoint_url(endpoint)
            .force_path_style(true) // Required for MinIO and most S3-compatible services
            .build();

        let client = Client::from_conf(s3_config);

        Ok(Self {
            client,
            bucket: bucket.into(),
            prefix,
        })
    }

    /// Create with explicit client (for testing).
    pub fn with_client(client: Client, bucket: impl Into<String>, prefix: Option<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
            prefix,
        }
    }

    /// Get the object key for a given hash.
    fn object_key(&self, hash: &[u8]) -> String {
        let hex = hash_to_hex(hash);
        let subdir = &hex[0..2];

        match &self.prefix {
            Some(prefix) => format!("{}/{}/{}", prefix, subdir, hex),
            None => format!("{}/{}", subdir, hex),
        }
    }

    /// Build a URI for an object.
    fn uri_for_object(&self, key: &str) -> String {
        format!("s3://{}/{}", self.bucket, key)
    }

    /// Extract object key from an s3:// URI.
    fn key_from_uri(&self, uri: &str) -> Result<String> {
        let expected_prefix = format!("s3://{}/", self.bucket);
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
impl PayloadStore for S3PayloadStore {
    async fn put(&self, payload: &[u8]) -> Result<PayloadReference> {
        let hash = compute_hash(payload);
        let key = self.object_key(&hash);

        // Check if already exists (deduplication)
        let exists = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .is_ok();

        if exists {
            debug!(
                hash = %hash_to_hex(&hash),
                "Payload already exists in S3, returning existing reference"
            );
        } else {
            // Upload the payload
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&key)
                .body(ByteStream::from(payload.to_vec()))
                .send()
                .await
                .map_err(|e| PayloadStoreError::StoreFailed(format!("S3 upload failed: {}", e)))?;

            debug!(
                hash = %hash_to_hex(&hash),
                size = payload.len(),
                bucket = %self.bucket,
                "Stored payload in S3"
            );
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();

        Ok(PayloadReference {
            storage_type: PayloadStorageType::S3 as i32,
            uri: self.uri_for_object(&key),
            content_hash: hash,
            original_size: payload.len() as u64,
            stored_at: Some(Timestamp {
                seconds: now.as_secs() as i64,
                nanos: now.subsec_nanos() as i32,
            }),
        })
    }

    async fn get(&self, reference: &PayloadReference) -> Result<Vec<u8>> {
        let key = self.key_from_uri(&reference.uri)?;

        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("NoSuchKey") || err_str.contains("404") {
                    PayloadStoreError::NotFound(reference.uri.clone())
                } else {
                    PayloadStoreError::RetrieveFailed(format!("S3 download failed: {}", e))
                }
            })?;

        let payload = response
            .body
            .collect()
            .await
            .map_err(|e| PayloadStoreError::RetrieveFailed(format!("S3 body read failed: {}", e)))?
            .into_bytes()
            .to_vec();

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
        let mut continuation_token: Option<String> = None;

        loop {
            let mut request = self.client.list_objects_v2().bucket(&self.bucket);

            if let Some(prefix) = &self.prefix {
                request = request.prefix(prefix);
            }

            if let Some(token) = &continuation_token {
                request = request.continuation_token(token);
            }

            let response = request
                .send()
                .await
                .map_err(|e| PayloadStoreError::StoreFailed(format!("S3 list failed: {}", e)))?;

            for object in response.contents() {
                // Check object last modified time
                if let Some(last_modified) = object.last_modified() {
                    let modified_secs = last_modified.secs() as u64;
                    if modified_secs < cutoff_secs {
                        if let Some(key) = object.key() {
                            if let Err(e) = self
                                .client
                                .delete_object()
                                .bucket(&self.bucket)
                                .key(key)
                                .send()
                                .await
                            {
                                warn!(
                                    key = %key,
                                    error = %e,
                                    "Failed to delete expired payload from S3"
                                );
                            } else {
                                deleted += 1;
                            }
                        }
                    }
                }
            }

            continuation_token = response.next_continuation_token().map(|s| s.to_string());
            if continuation_token.is_none() {
                break;
            }
        }

        Ok(deleted)
    }

    fn storage_type(&self) -> PayloadStorageType {
        PayloadStorageType::S3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_key_without_prefix() {
        let hash = compute_hash(b"test payload");
        let hex = hash_to_hex(&hash);

        // Key should be {hash[0:2]}/{hash}
        assert!(hex.len() >= 2);
        let expected = format!("{}/{}", &hex[0..2], hex);

        assert!(expected.contains('/'));
        assert!(expected.starts_with(&hex[0..2]));
    }

    #[test]
    fn test_object_key_with_prefix() {
        let hash = compute_hash(b"test payload");
        let hex = hash_to_hex(&hash);
        let prefix = "payloads";

        let expected = format!("{}/{}/{}", prefix, &hex[0..2], hex);

        assert!(expected.starts_with(prefix));
        assert!(expected.contains(&hex[0..2]));
    }

    #[test]
    fn test_uri_format() {
        let bucket = "my-bucket";
        let key = "ab/abcdef123456";
        let uri = format!("s3://{}/{}", bucket, key);

        assert!(uri.starts_with("s3://"));
        assert!(uri.contains(bucket));
    }
}
