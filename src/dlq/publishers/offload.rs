//! Offload storage DLQ publishers.
//!
//! Stores the ENTIRE dead letter message to remote storage (GCS, S3, filesystem).
//! Each backend is a separate self-registering publisher.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use prost::Message;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tracing::info;

use super::super::config::OffloadFilesystemDlqConfig;
#[cfg(feature = "gcs")]
use super::super::config::OffloadGcsDlqConfig;
#[cfg(feature = "s3")]
use super::super::config::OffloadS3DlqConfig;
use super::super::error::DlqError;
use super::super::factory::DlqBackend;
use super::super::{AngzarrDeadLetter, DeadLetterPublisher};

// ============================================================================
// Filesystem Offload
// ============================================================================

inventory::submit! {
    DlqBackend {
        try_create: |config| {
            let dlq_type = config.dlq_type.clone();
            let offload_config = config.offload_filesystem.clone();
            Box::pin(async move {
                if dlq_type != "offload-filesystem" {
                    return None;
                }
                let offload_config = offload_config.unwrap_or_default();
                match OffloadFilesystemDlqPublisher::new(&offload_config).await {
                    Ok(publisher) => Some(Ok(Arc::new(publisher) as Arc<dyn DeadLetterPublisher>)),
                    Err(e) => Some(Err(e)),
                }
            })
        },
    }
}

/// Filesystem-based offload storage for dead letters.
pub struct OffloadFilesystemDlqPublisher {
    base_path: PathBuf,
    prefix: String,
}

impl OffloadFilesystemDlqPublisher {
    /// Create a new filesystem offload publisher.
    pub async fn new(config: &OffloadFilesystemDlqConfig) -> Result<Self, DlqError> {
        let path = PathBuf::from(&config.base_path).join(&config.prefix);

        // Ensure directory exists
        fs::create_dir_all(&path).await.map_err(|e| {
            DlqError::Connection(format!(
                "Failed to create offload directory {:?}: {}",
                path, e
            ))
        })?;

        info!(
            base_path = %config.base_path,
            prefix = %config.prefix,
            "Filesystem offload DLQ publisher initialized"
        );

        Ok(Self {
            base_path: PathBuf::from(&config.base_path),
            prefix: config.prefix.clone(),
        })
    }

    /// Generate object key for a dead letter.
    fn generate_key(&self, dead_letter: &AngzarrDeadLetter) -> String {
        let timestamp = chrono::Utc::now().format("%Y/%m/%d/%H%M%S_%3f");
        let domain = dead_letter.domain().unwrap_or("unknown");
        let uuid = uuid::Uuid::new_v4();
        format!("{}{}/{}/{}.pb", self.prefix, domain, timestamp, uuid)
    }
}

#[async_trait]
impl DeadLetterPublisher for OffloadFilesystemDlqPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        let key = self.generate_key(&dead_letter);
        let file_path = self.base_path.join(&key);

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                DlqError::PublishFailed(format!("Failed to create directory {:?}: {}", parent, e))
            })?;
        }

        // Serialize the entire dead letter
        let proto = dead_letter.to_proto();
        let data = proto.encode_to_vec();

        let mut file = File::create(&file_path).await.map_err(|e| {
            DlqError::PublishFailed(format!("Failed to create file {:?}: {}", file_path, e))
        })?;

        file.write_all(&data).await.map_err(|e| {
            DlqError::PublishFailed(format!("Failed to write to file {:?}: {}", file_path, e))
        })?;

        file.flush().await.map_err(|e| {
            DlqError::PublishFailed(format!("Failed to flush file {:?}: {}", file_path, e))
        })?;

        info!(
            path = %file_path.display(),
            domain = %dead_letter.domain().unwrap_or("unknown"),
            reason = %dead_letter.rejection_reason,
            "Dead letter offloaded to filesystem"
        );

        #[cfg(feature = "otel")]
        {
            use crate::advice::metrics::{
                backend_attr, domain_attr, reason_type_attr, DLQ_PUBLISH_TOTAL,
            };
            DLQ_PUBLISH_TOTAL.add(
                1,
                &[
                    domain_attr(dead_letter.domain().unwrap_or("unknown")),
                    reason_type_attr(dead_letter.reason_type()),
                    backend_attr("offload_filesystem"),
                ],
            );
        }

        Ok(())
    }
}

// ============================================================================
// GCS Offload
// ============================================================================

#[cfg(feature = "gcs")]
inventory::submit! {
    DlqBackend {
        try_create: |config| {
            let dlq_type = config.dlq_type.clone();
            let offload_config = config.offload_gcs.clone();
            Box::pin(async move {
                if dlq_type != "offload-gcs" {
                    return None;
                }
                let Some(offload_config) = offload_config else {
                    return Some(Err(DlqError::NotConfigured));
                };
                match OffloadGcsDlqPublisher::new(&offload_config).await {
                    Ok(publisher) => Some(Ok(Arc::new(publisher) as Arc<dyn DeadLetterPublisher>)),
                    Err(e) => Some(Err(e)),
                }
            })
        },
    }
}

#[cfg(feature = "gcs")]
pub struct OffloadGcsDlqPublisher {
    client: gcloud_storage::client::Client,
    bucket: String,
    prefix: String,
}

#[cfg(feature = "gcs")]
impl OffloadGcsDlqPublisher {
    /// Create a new GCS offload publisher.
    pub async fn new(config: &OffloadGcsDlqConfig) -> Result<Self, DlqError> {
        let gcs_config = gcloud_storage::client::ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| DlqError::Connection(format!("Failed to configure GCS auth: {}", e)))?;

        let client = gcloud_storage::client::Client::new(gcs_config);

        info!(
            bucket = %config.bucket,
            prefix = %config.prefix,
            "GCS offload DLQ publisher initialized"
        );

        Ok(Self {
            client,
            bucket: config.bucket.clone(),
            prefix: config.prefix.clone(),
        })
    }

    /// Generate object key for a dead letter.
    fn generate_key(&self, dead_letter: &AngzarrDeadLetter) -> String {
        let timestamp = chrono::Utc::now().format("%Y/%m/%d/%H%M%S_%3f");
        let domain = dead_letter.domain().unwrap_or("unknown");
        let uuid = uuid::Uuid::new_v4();
        format!("{}{}/{}/{}.pb", self.prefix, domain, timestamp, uuid)
    }
}

#[cfg(feature = "gcs")]
#[async_trait]
impl DeadLetterPublisher for OffloadGcsDlqPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        let key = self.generate_key(&dead_letter);

        // Serialize the entire dead letter
        let proto = dead_letter.to_proto();
        let data = proto.encode_to_vec();

        let upload_type = gcloud_storage::http::objects::upload::UploadType::Simple(
            gcloud_storage::http::objects::upload::Media::new(key.clone()),
        );

        self.client
            .upload_object(
                &gcloud_storage::http::objects::upload::UploadObjectRequest {
                    bucket: self.bucket.clone(),
                    ..Default::default()
                },
                data,
                &upload_type,
            )
            .await
            .map_err(|e| DlqError::PublishFailed(format!("Failed to upload to GCS: {}", e)))?;

        info!(
            bucket = %self.bucket,
            key = %key,
            domain = %dead_letter.domain().unwrap_or("unknown"),
            reason = %dead_letter.rejection_reason,
            "Dead letter offloaded to GCS"
        );

        #[cfg(feature = "otel")]
        {
            use crate::advice::metrics::{
                backend_attr, domain_attr, reason_type_attr, DLQ_PUBLISH_TOTAL,
            };
            DLQ_PUBLISH_TOTAL.add(
                1,
                &[
                    domain_attr(dead_letter.domain().unwrap_or("unknown")),
                    reason_type_attr(dead_letter.reason_type()),
                    backend_attr("offload_gcs"),
                ],
            );
        }

        Ok(())
    }
}

// ============================================================================
// S3 Offload
// ============================================================================

#[cfg(feature = "s3")]
inventory::submit! {
    DlqBackend {
        try_create: |config| {
            let dlq_type = config.dlq_type.clone();
            let offload_config = config.offload_s3.clone();
            Box::pin(async move {
                if dlq_type != "offload-s3" {
                    return None;
                }
                let Some(offload_config) = offload_config else {
                    return Some(Err(DlqError::NotConfigured));
                };
                match OffloadS3DlqPublisher::new(&offload_config).await {
                    Ok(publisher) => Some(Ok(Arc::new(publisher) as Arc<dyn DeadLetterPublisher>)),
                    Err(e) => Some(Err(e)),
                }
            })
        },
    }
}

#[cfg(feature = "s3")]
pub struct OffloadS3DlqPublisher {
    client: aws_sdk_s3::Client,
    bucket: String,
    prefix: String,
}

#[cfg(feature = "s3")]
impl OffloadS3DlqPublisher {
    /// Create a new S3 offload publisher.
    pub async fn new(config: &OffloadS3DlqConfig) -> Result<Self, DlqError> {
        let mut aws_config_builder = aws_config::defaults(aws_config::BehaviorVersion::latest());

        if let Some(ref region) = config.region {
            aws_config_builder = aws_config_builder.region(aws_config::Region::new(region.clone()));
        }

        if let Some(ref endpoint) = config.endpoint_url {
            aws_config_builder = aws_config_builder.endpoint_url(endpoint);
        }

        let aws_config = aws_config_builder.load().await;
        let client = aws_sdk_s3::Client::new(&aws_config);

        info!(
            bucket = %config.bucket,
            prefix = %config.prefix,
            "S3 offload DLQ publisher initialized"
        );

        Ok(Self {
            client,
            bucket: config.bucket.clone(),
            prefix: config.prefix.clone(),
        })
    }

    /// Generate object key for a dead letter.
    fn generate_key(&self, dead_letter: &AngzarrDeadLetter) -> String {
        let timestamp = chrono::Utc::now().format("%Y/%m/%d/%H%M%S_%3f");
        let domain = dead_letter.domain().unwrap_or("unknown");
        let uuid = uuid::Uuid::new_v4();
        format!("{}{}/{}/{}.pb", self.prefix, domain, timestamp, uuid)
    }
}

#[cfg(feature = "s3")]
#[async_trait]
impl DeadLetterPublisher for OffloadS3DlqPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        let key = self.generate_key(&dead_letter);

        // Serialize the entire dead letter
        let proto = dead_letter.to_proto();
        let data = proto.encode_to_vec();

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(data.into())
            .content_type("application/x-protobuf")
            .send()
            .await
            .map_err(|e| DlqError::PublishFailed(format!("Failed to upload to S3: {}", e)))?;

        info!(
            bucket = %self.bucket,
            key = %key,
            domain = %dead_letter.domain().unwrap_or("unknown"),
            reason = %dead_letter.rejection_reason,
            "Dead letter offloaded to S3"
        );

        #[cfg(feature = "otel")]
        {
            use crate::advice::metrics::{
                backend_attr, domain_attr, reason_type_attr, DLQ_PUBLISH_TOTAL,
            };
            DLQ_PUBLISH_TOTAL.add(
                1,
                &[
                    domain_attr(dead_letter.domain().unwrap_or("unknown")),
                    reason_type_attr(dead_letter.reason_type()),
                    backend_attr("offload_s3"),
                ],
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dlq::{AngzarrDeadLetter, DeadLetterPayload, DeadLetterPublisher};
    use crate::proto::{
        command_page, CommandBook, CommandPage, Cover, MergeStrategy, Uuid as ProtoUuid,
    };
    use std::collections::HashMap;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn make_test_command(domain: &str) -> CommandBook {
        let root = Uuid::new_v4();
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: "test-corr-123".to_string(),
                edition: None,
                external_id: String::new(),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                payload: Some(command_page::Payload::Command(prost_types::Any {
                    type_url: "test.Command".to_string(),
                    value: vec![1, 2, 3],
                })),
                merge_strategy: MergeStrategy::MergeManual as i32,
            }],
            saga_origin: None,
        }
    }

    fn make_dead_letter(domain: &str, reason: &str) -> AngzarrDeadLetter {
        let cmd = make_test_command(domain);
        AngzarrDeadLetter {
            cover: cmd.cover.clone(),
            payload: DeadLetterPayload::Command(cmd),
            rejection_reason: reason.to_string(),
            rejection_details: None,
            occurred_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            metadata: HashMap::new(),
            source_component: "test-component".to_string(),
            source_component_type: "aggregate".to_string(),
        }
    }

    // NOTE: is_configured() and basic publish() tests are covered by
    // tests/interfaces/features/dlq_publishers.feature (Gherkin contract tests).
    // Only implementation-specific tests (path format, date structure, etc.) remain here.

    #[tokio::test]
    async fn test_offload_filesystem_file_path_includes_domain() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = OffloadFilesystemDlqConfig {
            base_path: temp_dir.path().to_string_lossy().to_string(),
            prefix: "dlq/".to_string(),
        };

        let publisher = OffloadFilesystemDlqPublisher::new(&config)
            .await
            .expect("Failed to create publisher");

        let dead_letter = make_dead_letter("inventory", "Test");
        publisher
            .publish(dead_letter)
            .await
            .expect("Failed to publish");

        // Check that domain directory exists
        let domain_base = temp_dir.path().join("dlq/inventory");
        assert!(
            domain_base.exists(),
            "Domain directory should exist: {:?}",
            domain_base
        );
    }

    #[test]
    fn test_generate_key_format() {
        let publisher = OffloadFilesystemDlqPublisher {
            base_path: PathBuf::from("/tmp"),
            prefix: "dlq/".to_string(),
        };

        let dead_letter = make_dead_letter("orders", "Test");
        let key = publisher.generate_key(&dead_letter);

        // Should start with prefix
        assert!(
            key.starts_with("dlq/"),
            "Key should start with prefix: {}",
            key
        );
        // Should contain domain
        assert!(key.contains("orders"), "Key should contain domain: {}", key);
        // Should end with .pb
        assert!(key.ends_with(".pb"), "Key should end with .pb: {}", key);
    }

    #[test]
    fn test_generate_key_includes_date_structure() {
        let publisher = OffloadFilesystemDlqPublisher {
            base_path: PathBuf::from("/tmp"),
            prefix: "".to_string(),
        };

        let dead_letter = make_dead_letter("orders", "Test");
        let key = publisher.generate_key(&dead_letter);

        // Should contain date structure (YYYY/MM/DD)
        let parts: Vec<&str> = key.split('/').collect();
        // Format: domain/YYYY/MM/DD/timestamp_uuid.pb
        assert!(
            parts.len() >= 5,
            "Should have date structure in path: {}",
            key
        );
    }
}
