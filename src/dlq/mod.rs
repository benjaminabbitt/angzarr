//! Dead Letter Queue (DLQ) infrastructure.
//!
//! DOC: This file is referenced in docs/docs/operations/error-recovery.mdx
//!      Update documentation when making changes to DLQ patterns.
//!
//! Provides a trait-based abstraction for publishing failed messages
//! to a dead letter queue for manual review and replay.
//!
//! ## Topic Naming
//!
//! DLQ topics follow the pattern: `angzarr.dlq.{domain}`
//!
//! This provides per-domain isolation for:
//! - Easier debugging (filter by domain)
//! - Domain-specific retention policies
//! - Domain-level access control
//!
//! ## Configuration
//!
//! DLQ is configured as a priority list of targets. Each target is tried in order
//! until one succeeds:
//!
//! ```yaml
//! dlq:
//!   targets:
//!     - type: amqp
//!       amqp:
//!         url: amqp://localhost:5672
//!     - type: database
//!       database:
//!         storage_type: postgres
//!     - type: logging
//! ```
//!
//! ## Message Format
//!
//! Uses `AngzarrDeadLetter` protobuf message which contains:
//! - Routing info (cover)
//! - Payload (oneof: rejected_command or rejected_events)
//! - Rejection details (oneof: sequence_mismatch, future types)
//! - Metadata (source component, timestamps)
//!
//! ## Usage
//!
//! ```ignore
//! // Initialize DLQ from config
//! let dlq_publisher = init_dlq_publisher(&config.dlq).await?;
//!
//! // On MERGE_MANUAL sequence mismatch
//! let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
//!     &command,
//!     expected,
//!     actual,
//!     MergeStrategy::MergeManual,
//!     "aggregate-name",
//! );
//! dlq_publisher.publish(dead_letter).await?;
//! ```

mod chained;
pub mod config;
pub mod error;
pub mod factory;
mod publishers;

use std::collections::HashMap;

use async_trait::async_trait;

use crate::proto::{
    angzarr_dead_letter, AngzarrDeadLetter as ProtoAngzarrDeadLetter, CommandBook, Cover,
    EventBook, EventProcessingFailedDetails as ProtoEventProcessingFailedDetails, MergeStrategy,
    PayloadRetrievalFailedDetails as ProtoPayloadRetrievalFailedDetails, PayloadStorageType,
    SequenceMismatchDetails as ProtoSequenceMismatchDetails,
};

// Re-export core types
pub use chained::ChainedDlqPublisher;
pub use config::{DlqConfig, DlqTargetConfig};
pub use error::{errmsg, DlqError};
pub use factory::{init_dlq_publisher, DlqBackend};

// Re-export publishers
pub use publishers::ChannelDeadLetterPublisher;
pub use publishers::FilesystemDeadLetterPublisher;
pub use publishers::LoggingDeadLetterPublisher;
pub use publishers::NoopDeadLetterPublisher;
pub use publishers::OffloadFilesystemDlqPublisher;

#[cfg(feature = "gcs")]
pub use publishers::OffloadGcsDlqPublisher;
#[cfg(feature = "s3")]
pub use publishers::OffloadS3DlqPublisher;

#[cfg(feature = "postgres")]
pub use publishers::PostgresDlqPublisher;
#[cfg(feature = "sqlite")]
pub use publishers::SqliteDlqPublisher;

#[cfg(feature = "amqp")]
pub use publishers::AmqpDeadLetterPublisher;
#[cfg(feature = "kafka")]
pub use publishers::KafkaDeadLetterPublisher;
#[cfg(feature = "pubsub")]
pub use publishers::PubSubDeadLetterPublisher;
#[cfg(feature = "sns-sqs")]
pub use publishers::SnsSqsDeadLetterPublisher;

/// DLQ topic prefix. Full topic: `{prefix}.{domain}`
pub const DLQ_TOPIC_PREFIX: &str = "angzarr.dlq";

/// Build the DLQ topic name for a domain.
pub fn dlq_topic_for_domain(domain: &str) -> String {
    format!("{}.{}", DLQ_TOPIC_PREFIX, domain)
}

/// Sequence mismatch details for DLQ entries.
///
/// Contains the expected vs actual sequence for debugging and replay.
#[derive(Debug, Clone)]
pub struct SequenceMismatchDetails {
    /// What the command expected.
    pub expected_sequence: u32,
    /// What the aggregate was actually at.
    pub actual_sequence: u32,
    /// Which merge strategy triggered the DLQ routing.
    pub merge_strategy: MergeStrategy,
}

/// Event processing failure details for DLQ entries.
///
/// Contains information about why a saga/projector failed to process events.
#[derive(Debug, Clone)]
pub struct EventProcessingFailedDetails {
    /// Error message from the handler.
    pub error: String,
    /// Number of retry attempts before DLQ routing.
    pub retry_count: u32,
    /// Whether the failure is considered transient (retry might succeed).
    pub is_transient: bool,
}

/// Payload retrieval failure details for DLQ entries.
///
/// Contains information about why an externally stored payload couldn't be retrieved.
#[derive(Debug, Clone)]
pub struct PayloadRetrievalFailedDetails {
    /// Storage backend type (filesystem, gcs, s3).
    pub storage_type: String,
    /// URI of the payload that couldn't be retrieved.
    pub uri: String,
    /// Content hash for identification.
    pub content_hash: Vec<u8>,
    /// Original payload size in bytes.
    pub original_size: u64,
    /// Error message from the retrieval attempt.
    pub error: String,
}

/// Payload types for dead letter entries.
#[derive(Debug, Clone)]
pub enum DeadLetterPayload {
    /// A command that failed to execute.
    Command(CommandBook),
    /// Events that failed to process (saga/projector failures).
    Events(EventBook),
}

/// Rejection details for dead letter entries.
///
/// Extensible via enum variants for future rejection types.
#[derive(Debug, Clone)]
pub enum RejectionDetails {
    /// Sequence mismatch with MERGE_MANUAL strategy.
    SequenceMismatch(SequenceMismatchDetails),
    /// Event processing failed in saga/projector handler.
    EventProcessingFailed(EventProcessingFailedDetails),
    /// Payload retrieval failed from external storage.
    PayloadRetrievalFailed(PayloadRetrievalFailedDetails),
}

/// Dead letter queue entry for failed messages.
///
/// This is the Rust representation of the AngzarrDeadLetter proto message.
/// When proto changes are made, this will be generated from proto.
#[derive(Debug, Clone)]
pub struct AngzarrDeadLetter {
    /// Routing info: domain, root, correlation_id.
    pub cover: Option<Cover>,
    /// The failed payload.
    pub payload: DeadLetterPayload,
    /// Human-readable reason for rejection.
    pub rejection_reason: String,
    /// Structured rejection details.
    pub rejection_details: Option<RejectionDetails>,
    /// When the rejection occurred.
    pub occurred_at: Option<prost_types::Timestamp>,
    /// Additional context.
    pub metadata: HashMap<String, String>,
    /// Which component sent to DLQ.
    pub source_component: String,
    /// Component type: "aggregate", "saga", "projector", "process_manager".
    pub source_component_type: String,
}

impl AngzarrDeadLetter {
    /// Create a dead letter from a sequence mismatch on a command.
    pub fn from_sequence_mismatch(
        command: &CommandBook,
        expected: u32,
        actual: u32,
        strategy: MergeStrategy,
        source_component: &str,
    ) -> Self {
        let reason = format!(
            "Sequence mismatch: command expects {}, aggregate at {}",
            expected, actual
        );

        Self {
            cover: command.cover.clone(),
            payload: DeadLetterPayload::Command(command.clone()),
            rejection_reason: reason,
            rejection_details: Some(RejectionDetails::SequenceMismatch(
                SequenceMismatchDetails {
                    expected_sequence: expected,
                    actual_sequence: actual,
                    merge_strategy: strategy,
                },
            )),
            occurred_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            metadata: HashMap::new(),
            source_component: source_component.to_string(),
            source_component_type: "aggregate".to_string(),
        }
    }

    /// Create a dead letter from failed event processing.
    pub fn from_event_processing_failure(
        events: &EventBook,
        error: &str,
        retry_count: u32,
        is_transient: bool,
        source_component: &str,
        source_component_type: &str,
    ) -> Self {
        let reason = format!(
            "Event processing failed after {} attempts: {}",
            retry_count, error
        );

        Self {
            cover: events.cover.clone(),
            payload: DeadLetterPayload::Events(events.clone()),
            rejection_reason: reason,
            rejection_details: Some(RejectionDetails::EventProcessingFailed(
                EventProcessingFailedDetails {
                    error: error.to_string(),
                    retry_count,
                    is_transient,
                },
            )),
            occurred_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            metadata: HashMap::new(),
            source_component: source_component.to_string(),
            source_component_type: source_component_type.to_string(),
        }
    }

    /// Create a dead letter from a payload retrieval failure.
    ///
    /// Used when externally stored payloads (claim check pattern) cannot be retrieved.
    pub fn from_payload_retrieval_failure(
        events: &EventBook,
        storage_type: &str,
        uri: &str,
        content_hash: &[u8],
        original_size: u64,
        error: &str,
        source_component: &str,
    ) -> Self {
        let reason = format!(
            "Payload retrieval failed from {} ({}): {}",
            storage_type, uri, error
        );

        Self {
            cover: events.cover.clone(),
            payload: DeadLetterPayload::Events(events.clone()),
            rejection_reason: reason,
            rejection_details: Some(RejectionDetails::PayloadRetrievalFailed(
                PayloadRetrievalFailedDetails {
                    storage_type: storage_type.to_string(),
                    uri: uri.to_string(),
                    content_hash: content_hash.to_vec(),
                    original_size,
                    error: error.to_string(),
                },
            )),
            occurred_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            metadata: HashMap::new(),
            source_component: source_component.to_string(),
            source_component_type: "bus".to_string(), // Payload retrieval happens at bus layer
        }
    }

    /// Add metadata to the dead letter.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Get the domain for DLQ topic routing.
    pub fn domain(&self) -> Option<&str> {
        self.cover.as_ref().map(|c| c.domain.as_str())
    }

    /// Get the DLQ topic for this dead letter.
    pub fn topic(&self) -> String {
        let domain = self.domain().unwrap_or("unknown");
        dlq_topic_for_domain(domain)
    }

    /// Get the reason type for metrics labeling.
    pub fn reason_type(&self) -> &'static str {
        match &self.rejection_details {
            Some(RejectionDetails::SequenceMismatch(_)) => "sequence_mismatch",
            Some(RejectionDetails::EventProcessingFailed(_)) => "event_processing_failed",
            Some(RejectionDetails::PayloadRetrievalFailed(_)) => "payload_retrieval_failed",
            None => "unknown",
        }
    }

    /// Convert to proto representation for serialization.
    pub fn to_proto(&self) -> ProtoAngzarrDeadLetter {
        let payload = match &self.payload {
            DeadLetterPayload::Command(cmd) => {
                Some(angzarr_dead_letter::Payload::RejectedCommand(cmd.clone()))
            }
            DeadLetterPayload::Events(events) => {
                Some(angzarr_dead_letter::Payload::RejectedEvents(events.clone()))
            }
        };

        let rejection_details = self
            .rejection_details
            .as_ref()
            .map(|details| match details {
                RejectionDetails::SequenceMismatch(d) => {
                    angzarr_dead_letter::RejectionDetails::SequenceMismatch(
                        ProtoSequenceMismatchDetails {
                            expected_sequence: d.expected_sequence,
                            actual_sequence: d.actual_sequence,
                            merge_strategy: d.merge_strategy as i32,
                        },
                    )
                }
                RejectionDetails::EventProcessingFailed(d) => {
                    angzarr_dead_letter::RejectionDetails::EventProcessingFailed(
                        ProtoEventProcessingFailedDetails {
                            error: d.error.clone(),
                            retry_count: d.retry_count,
                            is_transient: d.is_transient,
                        },
                    )
                }
                RejectionDetails::PayloadRetrievalFailed(d) => {
                    let storage_type = match d.storage_type.as_str() {
                        "filesystem" => PayloadStorageType::Filesystem,
                        "gcs" => PayloadStorageType::Gcs,
                        "s3" => PayloadStorageType::S3,
                        _ => PayloadStorageType::Unspecified,
                    };
                    angzarr_dead_letter::RejectionDetails::PayloadRetrievalFailed(
                        ProtoPayloadRetrievalFailedDetails {
                            storage_type: storage_type as i32,
                            uri: d.uri.clone(),
                            content_hash: d.content_hash.clone(),
                            original_size: d.original_size,
                            error: d.error.clone(),
                        },
                    )
                }
            });

        ProtoAngzarrDeadLetter {
            cover: self.cover.clone(),
            payload,
            rejection_reason: self.rejection_reason.clone(),
            rejection_details,
            occurred_at: self.occurred_at,
            metadata: self.metadata.clone(),
            source_component: self.source_component.clone(),
            source_component_type: self.source_component_type.clone(),
        }
    }
}

/// Trait for publishing messages to a dead letter queue.
///
/// Implementations handle the actual transport (AMQP, Kafka, in-memory, etc.).
#[async_trait]
pub trait DeadLetterPublisher: Send + Sync {
    /// Publish a dead letter to the queue.
    ///
    /// Returns Ok(()) on successful publish, Err on failure.
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError>;

    /// Check if the publisher is configured and ready.
    fn is_configured(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{command_page, CommandPage, Uuid as ProtoUuid};
    use uuid::Uuid;

    fn make_test_command(domain: &str, root: Uuid) -> CommandBook {
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

    // ============================================================================
    // Topic Naming Tests
    // ============================================================================

    #[test]
    fn test_dlq_topic_for_domain() {
        assert_eq!(dlq_topic_for_domain("orders"), "angzarr.dlq.orders");
        assert_eq!(dlq_topic_for_domain("inventory"), "angzarr.dlq.inventory");
        assert_eq!(dlq_topic_for_domain("player"), "angzarr.dlq.player");
    }

    #[test]
    fn test_dead_letter_topic() {
        let cmd = make_test_command("orders", Uuid::new_v4());
        let dl = AngzarrDeadLetter::from_sequence_mismatch(
            &cmd,
            0,
            5,
            MergeStrategy::MergeManual,
            "test-agg",
        );
        assert_eq!(dl.topic(), "angzarr.dlq.orders");
    }

    // ============================================================================
    // Dead Letter Creation Tests
    // ============================================================================

    #[test]
    fn test_from_sequence_mismatch() {
        let root = Uuid::new_v4();
        let cmd = make_test_command("orders", root);

        let dl = AngzarrDeadLetter::from_sequence_mismatch(
            &cmd,
            0,
            5,
            MergeStrategy::MergeManual,
            "orders-agg",
        );

        assert_eq!(dl.domain(), Some("orders"));
        assert!(dl.rejection_reason.contains("0"));
        assert!(dl.rejection_reason.contains("5"));
        assert_eq!(dl.source_component, "orders-agg");
        assert_eq!(dl.source_component_type, "aggregate");

        match &dl.rejection_details {
            Some(RejectionDetails::SequenceMismatch(details)) => {
                assert_eq!(details.expected_sequence, 0);
                assert_eq!(details.actual_sequence, 5);
                assert_eq!(details.merge_strategy, MergeStrategy::MergeManual);
            }
            _ => panic!("Expected SequenceMismatch details"),
        }

        match &dl.payload {
            DeadLetterPayload::Command(c) => {
                assert_eq!(c.cover.as_ref().unwrap().domain, "orders");
            }
            _ => panic!("Expected Command payload"),
        }
    }

    #[test]
    fn test_from_event_processing_failure() {
        let root = Uuid::new_v4();
        let events = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: "test-corr".to_string(),
                edition: None,
                external_id: String::new(),
            }),
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };

        let dl = AngzarrDeadLetter::from_event_processing_failure(
            &events,
            "Saga handler failed",
            3,
            false,
            "saga-order-fulfillment",
            "saga",
        );

        assert_eq!(dl.domain(), Some("orders"));
        assert!(dl.rejection_reason.contains("Saga handler failed"));
        assert!(dl.rejection_reason.contains("3 attempts"));
        assert_eq!(dl.source_component, "saga-order-fulfillment");
        assert_eq!(dl.source_component_type, "saga");

        match &dl.rejection_details {
            Some(RejectionDetails::EventProcessingFailed(details)) => {
                assert_eq!(details.error, "Saga handler failed");
                assert_eq!(details.retry_count, 3);
                assert!(!details.is_transient);
            }
            _ => panic!("Expected EventProcessingFailed details"),
        }

        match &dl.payload {
            DeadLetterPayload::Events(_) => {}
            _ => panic!("Expected Events payload"),
        }
    }

    #[test]
    fn test_with_metadata() {
        let cmd = make_test_command("orders", Uuid::new_v4());
        let dl = AngzarrDeadLetter::from_sequence_mismatch(
            &cmd,
            0,
            5,
            MergeStrategy::MergeManual,
            "test-agg",
        )
        .with_metadata("retry_count", "3")
        .with_metadata("original_timestamp", "2024-01-01T00:00:00Z");

        assert_eq!(dl.metadata.get("retry_count"), Some(&"3".to_string()));
        assert_eq!(
            dl.metadata.get("original_timestamp"),
            Some(&"2024-01-01T00:00:00Z".to_string())
        );
    }

    // ============================================================================
    // Noop Publisher Tests
    // ============================================================================

    #[tokio::test]
    async fn test_noop_publisher_succeeds() {
        let publisher = NoopDeadLetterPublisher;
        let cmd = make_test_command("orders", Uuid::new_v4());
        let dl = AngzarrDeadLetter::from_sequence_mismatch(
            &cmd,
            0,
            5,
            MergeStrategy::MergeManual,
            "test-agg",
        );

        let result = publisher.publish(dl).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_noop_publisher_not_configured() {
        let publisher = NoopDeadLetterPublisher;
        assert!(!publisher.is_configured());
    }

    // ============================================================================
    // Channel Publisher Tests
    // ============================================================================

    #[tokio::test]
    async fn test_channel_publisher_sends() {
        let (publisher, mut receiver) = ChannelDeadLetterPublisher::new();
        let cmd = make_test_command("orders", Uuid::new_v4());
        let dl = AngzarrDeadLetter::from_sequence_mismatch(
            &cmd,
            0,
            5,
            MergeStrategy::MergeManual,
            "test-agg",
        );

        publisher.publish(dl).await.unwrap();

        let received = receiver.recv().await.expect("Should receive dead letter");
        assert_eq!(received.domain(), Some("orders"));
        assert_eq!(received.source_component, "test-agg");
    }

    // ============================================================================
    // Config Tests
    // ============================================================================

    #[test]
    fn test_dlq_config_default_not_configured() {
        let config = DlqConfig::default();
        assert!(!config.is_configured());
    }

    #[test]
    fn test_dlq_config_amqp_configured() {
        let config = DlqConfig::amqp("amqp://localhost:5672");
        assert!(config.is_configured());
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0].dlq_type, "amqp");
    }

    #[test]
    fn test_dlq_config_kafka_configured() {
        let config = DlqConfig::kafka("localhost:9092");
        assert!(config.is_configured());
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0].dlq_type, "kafka");
    }

    // ============================================================================
    // Error Tests
    // ============================================================================

    #[test]
    fn test_dlq_error_display() {
        let err = DlqError::NotConfigured;
        assert_eq!(err.to_string(), errmsg::NOT_CONFIGURED);

        let err = DlqError::PublishFailed("connection refused".to_string());
        assert_eq!(
            err.to_string(),
            format!("{}connection refused", errmsg::PUBLISH_FAILED)
        );

        let err = DlqError::UnknownType("custom".to_string());
        assert_eq!(err.to_string(), format!("{}custom", errmsg::UNKNOWN_TYPE));
    }

    // ============================================================================
    // Proto Conversion Tests
    // ============================================================================

    #[test]
    fn test_to_proto_sequence_mismatch() {
        let cmd = make_test_command("orders", Uuid::new_v4());
        let dl = AngzarrDeadLetter::from_sequence_mismatch(
            &cmd,
            5,
            10,
            MergeStrategy::MergeManual,
            "test-agg",
        );

        let proto = dl.to_proto();

        assert!(proto.cover.is_some());
        assert!(proto.payload.is_some());
        assert!(proto.rejection_details.is_some());
        assert!(!proto.rejection_reason.is_empty());
        assert_eq!(proto.source_component, "test-agg");
        assert_eq!(proto.source_component_type, "aggregate");
    }

    // ============================================================================
    // Reason Type Tests
    // ============================================================================

    #[test]
    fn test_reason_type_sequence_mismatch() {
        let cmd = make_test_command("orders", Uuid::new_v4());
        let dl = AngzarrDeadLetter::from_sequence_mismatch(
            &cmd,
            0,
            5,
            MergeStrategy::MergeManual,
            "test",
        );
        assert_eq!(dl.reason_type(), "sequence_mismatch");
    }

    #[test]
    fn test_reason_type_event_processing_failed() {
        let events = EventBook::default();
        let dl = AngzarrDeadLetter::from_event_processing_failure(
            &events, "err", 1, false, "saga", "saga",
        );
        assert_eq!(dl.reason_type(), "event_processing_failed");
    }

    #[test]
    fn test_reason_type_unknown() {
        let dl = AngzarrDeadLetter {
            cover: None,
            payload: DeadLetterPayload::Events(EventBook::default()),
            rejection_reason: "Unknown reason".to_string(),
            rejection_details: None,
            occurred_at: None,
            metadata: HashMap::new(),
            source_component: "test".to_string(),
            source_component_type: "test".to_string(),
        };
        assert_eq!(dl.reason_type(), "unknown");
    }
}
