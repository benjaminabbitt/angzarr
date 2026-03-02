//! Filesystem-based DLQ publisher.
//!
//! Writes dead letters to files in a configured directory.
//! Useful for development, debugging, and local persistence.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use prost::Message;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tracing::info;

use super::super::config::FilesystemDlqConfig;
use super::super::error::DlqError;
use super::super::factory::DlqBackend;
use super::super::{AngzarrDeadLetter, DeadLetterPublisher};

// ============================================================================
// Self-Registration
// ============================================================================

inventory::submit! {
    DlqBackend {
        try_create: |config| {
            let dlq_type = config.dlq_type.clone();
            let fs_config = config.filesystem.clone();
            Box::pin(async move {
                if dlq_type != "filesystem" {
                    return None;
                }
                let fs_config = fs_config.unwrap_or_default();
                match FilesystemDeadLetterPublisher::new(&fs_config).await {
                    Ok(publisher) => Some(Ok(Arc::new(publisher) as Arc<dyn DeadLetterPublisher>)),
                    Err(e) => Some(Err(e)),
                }
            })
        },
    }
}

/// Filesystem-based DLQ publisher.
///
/// Writes dead letters to files in a configured directory.
/// File naming: `{timestamp}_{domain}_{correlation_id}.{format}`
pub struct FilesystemDeadLetterPublisher {
    path: PathBuf,
    format: String,
}

impl FilesystemDeadLetterPublisher {
    /// Create a new filesystem DLQ publisher.
    pub async fn new(config: &FilesystemDlqConfig) -> Result<Self, DlqError> {
        let path = PathBuf::from(&config.path);

        // Ensure directory exists
        fs::create_dir_all(&path).await.map_err(|e| {
            DlqError::Connection(format!("Failed to create DLQ directory {:?}: {}", path, e))
        })?;

        info!(path = %config.path, format = %config.format, "Filesystem DLQ publisher initialized");

        Ok(Self {
            path,
            format: config.format.clone(),
        })
    }

    /// Generate filename for a dead letter.
    fn generate_filename(&self, dead_letter: &AngzarrDeadLetter) -> String {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f");
        let domain = dead_letter.domain().unwrap_or("unknown");
        let correlation_id = dead_letter
            .cover
            .as_ref()
            .map(|c| {
                if c.correlation_id.is_empty() {
                    "no-correlation".to_string()
                } else {
                    // Sanitize correlation ID for filename
                    c.correlation_id
                        .chars()
                        .map(|c| {
                            if c.is_alphanumeric() || c == '-' || c == '_' {
                                c
                            } else {
                                '_'
                            }
                        })
                        .collect()
                }
            })
            .unwrap_or_else(|| "no-correlation".to_string());

        let ext = match self.format.as_str() {
            "protobuf" | "proto" => "pb",
            _ => "json",
        };

        format!(
            "{}_{}_{}_{}.{}",
            timestamp,
            domain,
            correlation_id,
            uuid::Uuid::new_v4().as_simple(),
            ext
        )
    }
}

#[async_trait]
impl DeadLetterPublisher for FilesystemDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        let filename = self.generate_filename(&dead_letter);
        let file_path = self.path.join(&filename);

        let data = match self.format.as_str() {
            "protobuf" | "proto" => {
                let proto = dead_letter.to_proto();
                proto.encode_to_vec()
            }
            _ => {
                // JSON format - serialize as human-readable JSON
                // Proto messages don't have serde derives, so we create a JSON manually
                let proto_bytes = dead_letter.to_proto().encode_to_vec();
                let payload_base64 = base64::engine::general_purpose::STANDARD.encode(&proto_bytes);

                let json = serde_json::json!({
                    "domain": dead_letter.domain().unwrap_or("unknown"),
                    "correlation_id": dead_letter.cover.as_ref().map(|c| &c.correlation_id),
                    "rejection_reason": dead_letter.rejection_reason,
                    "reason_type": dead_letter.reason_type(),
                    "source_component": dead_letter.source_component,
                    "source_component_type": dead_letter.source_component_type,
                    "metadata": dead_letter.metadata,
                    "occurred_at": dead_letter.occurred_at.as_ref().map(|ts| {
                        chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
                            .map(|dt| dt.to_rfc3339())
                    }),
                    // Include proto bytes as base64 for recovery
                    "payload_base64": payload_base64,
                });
                serde_json::to_vec_pretty(&json).map_err(|e| {
                    DlqError::Serialization(format!("Failed to serialize to JSON: {}", e))
                })?
            }
        };

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
            "Dead letter written to filesystem"
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
                    backend_attr("filesystem"),
                ],
            );
        }

        Ok(())
    }

    fn is_configured(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    //! Tests for filesystem-based DLQ publisher.
    //!
    //! The filesystem publisher writes dead letters to files for debugging and
    //! local persistence. Tests here focus on implementation-specific behaviors:
    //! - Filename generation (domain, timestamp, sanitized correlation ID)
    //! - Format-specific output (JSON vs protobuf)
    //! - Character sanitization in filenames
    //!
    //! Basic publish and is_configured() tests are covered by Gherkin contract
    //! tests in tests/interfaces/features/dlq_publishers.feature.

    use super::*;
    use crate::dlq::{AngzarrDeadLetter, DeadLetterPayload};
    use crate::proto::{
        command_page, CommandBook, CommandPage, Cover, MergeStrategy, Uuid as ProtoUuid,
    };
    use std::collections::HashMap;
    use tempfile::TempDir;
    use uuid::Uuid;

    // ============================================================================
    // Test Helpers
    // ============================================================================

    fn make_test_command(domain: &str, correlation_id: &str) -> CommandBook {
        let root = Uuid::new_v4();
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: correlation_id.to_string(),
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

    fn make_dead_letter(domain: &str, correlation_id: &str, reason: &str) -> AngzarrDeadLetter {
        let cmd = make_test_command(domain, correlation_id);
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

    // ============================================================================
    // Filename Generation Tests
    // ============================================================================

    /// Filename includes domain for easy filtering/grep.
    #[tokio::test]
    async fn test_filesystem_publisher_filename_includes_domain() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = FilesystemDlqConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            format: "json".to_string(),
            max_files: 0,
        };

        let publisher = FilesystemDeadLetterPublisher::new(&config)
            .await
            .expect("Failed to create publisher");

        let dead_letter = make_dead_letter("inventory", "corr-abc", "Test");
        publisher
            .publish(dead_letter)
            .await
            .expect("Failed to publish");

        // Check filename contains domain
        let mut entries = fs::read_dir(temp_dir.path())
            .await
            .expect("Failed to read dir");
        let entry = entries
            .next_entry()
            .await
            .expect("Failed to read entry")
            .expect("No file found");
        let filename = entry.file_name().to_string_lossy().to_string();

        assert!(
            filename.contains("inventory"),
            "Filename should contain domain: {}",
            filename
        );
    }

    // ============================================================================
    // Format Tests
    // ============================================================================

    /// JSON format includes all fields needed for debugging/recovery.
    #[tokio::test]
    async fn test_filesystem_publisher_json_format_contains_fields() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = FilesystemDlqConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            format: "json".to_string(),
            max_files: 0,
        };

        let publisher = FilesystemDeadLetterPublisher::new(&config)
            .await
            .expect("Failed to create publisher");

        let dead_letter = make_dead_letter("orders", "test-corr", "Sequence mismatch");
        publisher
            .publish(dead_letter)
            .await
            .expect("Failed to publish");

        // Read file and parse JSON
        let mut entries = fs::read_dir(temp_dir.path())
            .await
            .expect("Failed to read dir");
        let entry = entries
            .next_entry()
            .await
            .expect("Failed to read entry")
            .expect("No file found");
        let content = fs::read_to_string(entry.path())
            .await
            .expect("Failed to read file");
        let json: serde_json::Value = serde_json::from_str(&content).expect("Failed to parse JSON");

        assert_eq!(json["domain"], "orders");
        assert_eq!(json["rejection_reason"], "Sequence mismatch");
        assert!(
            json["payload_base64"].is_string(),
            "Should have base64 payload"
        );
    }

    /// Protobuf format writes binary (not JSON) for compact storage.
    #[tokio::test]
    async fn test_filesystem_publisher_protobuf_format_writes_binary() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = FilesystemDlqConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            format: "protobuf".to_string(),
            max_files: 0,
        };

        let publisher = FilesystemDeadLetterPublisher::new(&config)
            .await
            .expect("Failed to create publisher");

        let dead_letter = make_dead_letter("orders", "test-corr", "Test");
        publisher
            .publish(dead_letter)
            .await
            .expect("Failed to publish");

        // Read file and verify it's binary (not valid UTF-8 JSON)
        let mut entries = fs::read_dir(temp_dir.path())
            .await
            .expect("Failed to read dir");
        let entry = entries
            .next_entry()
            .await
            .expect("Failed to read entry")
            .expect("No file found");
        let content = fs::read(entry.path()).await.expect("Failed to read file");

        // Protobuf should not be valid JSON
        assert!(
            serde_json::from_slice::<serde_json::Value>(&content).is_err(),
            "Protobuf format should not be valid JSON"
        );
        // But should have some data
        assert!(!content.is_empty(), "File should not be empty");
    }

    // ============================================================================
    // Filename Sanitization Tests
    // ============================================================================

    /// Special characters in correlation ID replaced with underscore.
    ///
    /// Prevents path traversal and invalid filenames. Correlation IDs from
    /// external systems may contain arbitrary characters.
    #[test]
    fn test_generate_filename_sanitizes_correlation_id() {
        let publisher = FilesystemDeadLetterPublisher {
            path: PathBuf::from("/tmp"),
            format: "json".to_string(),
        };

        // Create dead letter with special chars in correlation ID
        let mut dead_letter = make_dead_letter("orders", "test/corr:123?bad", "Test");
        dead_letter.cover.as_mut().unwrap().correlation_id = "test/corr:123?bad".to_string();

        let filename = publisher.generate_filename(&dead_letter);

        // Should not contain special chars
        assert!(!filename.contains('/'), "Filename should not contain /");
        assert!(!filename.contains(':'), "Filename should not contain :");
        assert!(!filename.contains('?'), "Filename should not contain ?");
        assert!(filename.contains(".json"), "Should have .json extension");
    }

    /// Hyphens preserved — common in UUIDs and readable IDs.
    #[test]
    fn test_generate_filename_preserves_hyphens() {
        let publisher = FilesystemDeadLetterPublisher {
            path: PathBuf::from("/tmp"),
            format: "json".to_string(),
        };

        // Correlation ID with hyphens should be preserved
        let mut dead_letter = make_dead_letter("orders", "test-corr-123", "Test");
        dead_letter.cover.as_mut().unwrap().correlation_id = "test-corr-123".to_string();

        let filename = publisher.generate_filename(&dead_letter);

        // Verify hyphen is preserved (not replaced with _)
        assert!(
            filename.contains("test-corr-123"),
            "Filename should preserve hyphens: {}",
            filename
        );
    }

    /// Underscores preserved — common in snake_case IDs.
    #[test]
    fn test_generate_filename_preserves_underscores() {
        let publisher = FilesystemDeadLetterPublisher {
            path: PathBuf::from("/tmp"),
            format: "json".to_string(),
        };

        // Correlation ID with underscores should be preserved
        let mut dead_letter = make_dead_letter("orders", "test_corr_123", "Test");
        dead_letter.cover.as_mut().unwrap().correlation_id = "test_corr_123".to_string();

        let filename = publisher.generate_filename(&dead_letter);

        // Verify underscore is preserved
        assert!(
            filename.contains("test_corr_123"),
            "Filename should preserve underscores: {}",
            filename
        );
    }

    // ============================================================================
    // Extension Tests
    // ============================================================================

    /// Protobuf format uses .pb extension.
    #[test]
    fn test_generate_filename_protobuf_extension() {
        let publisher = FilesystemDeadLetterPublisher {
            path: PathBuf::from("/tmp"),
            format: "protobuf".to_string(),
        };

        let dead_letter = make_dead_letter("orders", "test-corr", "Test");
        let filename = publisher.generate_filename(&dead_letter);

        assert!(
            filename.ends_with(".pb"),
            "Protobuf format should have .pb extension: {}",
            filename
        );
    }

    /// "proto" format alias also uses .pb extension.
    #[test]
    fn test_generate_filename_proto_extension() {
        let publisher = FilesystemDeadLetterPublisher {
            path: PathBuf::from("/tmp"),
            format: "proto".to_string(),
        };

        let dead_letter = make_dead_letter("orders", "test-corr", "Test");
        let filename = publisher.generate_filename(&dead_letter);

        assert!(
            filename.ends_with(".pb"),
            "Proto format should have .pb extension: {}",
            filename
        );
    }
}
