//! Dead Letter Queue (DLQ) infrastructure.
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
//! // In coordinator initialization
//! let dlq_publisher = AmqpDeadLetterPublisher::new(config).await?;
//!
//! // On MERGE_MANUAL sequence mismatch
//! let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
//!     &command,
//!     expected,
//!     actual,
//!     MergeStrategy::MergeManual,  // TODO: Add this enum value to proto
//! );
//! dlq_publisher.publish(dead_letter).await?;
//! ```

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::proto::{CommandBook, Cover, EventBook, MergeStrategy};

/// DLQ topic prefix. Full topic: `{prefix}.{domain}`
pub const DLQ_TOPIC_PREFIX: &str = "angzarr.dlq";

/// Build the DLQ topic name for a domain.
pub fn dlq_topic_for_domain(domain: &str) -> String {
    format!("{}.{}", DLQ_TOPIC_PREFIX, domain)
}

/// Errors that can occur during DLQ operations.
#[derive(Debug, thiserror::Error)]
pub enum DlqError {
    #[error("DLQ not configured")]
    NotConfigured,

    #[error("Failed to serialize message: {0}")]
    Serialization(String),

    #[error("Failed to publish to DLQ: {0}")]
    PublishFailed(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Invalid dead letter: {0}")]
    InvalidDeadLetter(String),
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

/// No-op DLQ publisher that logs but doesn't actually send anywhere.
///
/// Used when DLQ is not configured or for testing.
pub struct NoopDeadLetterPublisher;

#[async_trait]
impl DeadLetterPublisher for NoopDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        warn!(
            topic = %dead_letter.topic(),
            reason = %dead_letter.rejection_reason,
            source = %dead_letter.source_component,
            "DLQ not configured, logging dead letter"
        );
        Ok(())
    }

    fn is_configured(&self) -> bool {
        false
    }
}

/// In-memory DLQ publisher using a channel.
///
/// Used for standalone mode and testing.
pub struct ChannelDeadLetterPublisher {
    sender: mpsc::UnboundedSender<AngzarrDeadLetter>,
}

impl ChannelDeadLetterPublisher {
    /// Create a new channel-based DLQ publisher.
    ///
    /// Returns the publisher and a receiver for consuming dead letters.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<AngzarrDeadLetter>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }
}

#[async_trait]
impl DeadLetterPublisher for ChannelDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        info!(
            topic = %dead_letter.topic(),
            reason = %dead_letter.rejection_reason,
            "Publishing to channel DLQ"
        );
        self.sender
            .send(dead_letter)
            .map_err(|e| DlqError::PublishFailed(e.to_string()))
    }
}

/// Configuration for DLQ publishers.
#[derive(Debug, Clone, Default)]
pub struct DlqConfig {
    /// AMQP connection URL (for AMQP publisher).
    pub amqp_url: Option<String>,
    /// Kafka bootstrap servers (for Kafka publisher).
    pub kafka_brokers: Option<String>,
    /// Whether to use per-domain topics.
    pub per_domain_topics: bool,
}

impl DlqConfig {
    /// Check if any DLQ backend is configured.
    pub fn is_configured(&self) -> bool {
        self.amqp_url.is_some() || self.kafka_brokers.is_some()
    }
}

/// Create a DLQ publisher based on configuration.
///
/// Returns NoopDeadLetterPublisher if nothing is configured.
pub fn create_publisher(config: &DlqConfig) -> Arc<dyn DeadLetterPublisher> {
    if config.amqp_url.is_some() {
        // TODO: Return AmqpDeadLetterPublisher when implemented
        warn!("AMQP DLQ configured but not implemented, using noop");
        Arc::new(NoopDeadLetterPublisher)
    } else if config.kafka_brokers.is_some() {
        // TODO: Return KafkaDeadLetterPublisher when implemented
        warn!("Kafka DLQ configured but not implemented, using noop");
        Arc::new(NoopDeadLetterPublisher)
    } else {
        debug!("No DLQ configured, using noop publisher");
        Arc::new(NoopDeadLetterPublisher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{CommandPage, Uuid as ProtoUuid};
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
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(prost_types::Any {
                    type_url: "test.Command".to_string(),
                    value: vec![1, 2, 3],
                }),
                merge_strategy: MergeStrategy::MergeManual as i32,
                external_payload: None,
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
    fn test_from_payload_retrieval_failure() {
        let root = Uuid::new_v4();
        let events = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: "test-corr".to_string(),
                edition: None,
            }),
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };

        let hash = vec![0xab, 0xcd, 0xef];
        let dl = AngzarrDeadLetter::from_payload_retrieval_failure(
            &events,
            "gcs",
            "gs://bucket/payloads/abc123.bin",
            &hash,
            1024,
            "Object not found",
            "offloading-bus",
        );

        assert_eq!(dl.domain(), Some("orders"));
        assert!(dl.rejection_reason.contains("gcs"));
        assert!(dl.rejection_reason.contains("Object not found"));
        assert_eq!(dl.source_component, "offloading-bus");
        assert_eq!(dl.source_component_type, "bus");

        match &dl.rejection_details {
            Some(RejectionDetails::PayloadRetrievalFailed(details)) => {
                assert_eq!(details.storage_type, "gcs");
                assert_eq!(details.uri, "gs://bucket/payloads/abc123.bin");
                assert_eq!(details.content_hash, vec![0xab, 0xcd, 0xef]);
                assert_eq!(details.original_size, 1024);
                assert_eq!(details.error, "Object not found");
            }
            _ => panic!("Expected PayloadRetrievalFailed details"),
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

    #[tokio::test]
    async fn test_channel_publisher_multiple() {
        let (publisher, mut receiver) = ChannelDeadLetterPublisher::new();

        for i in 0..3 {
            let cmd = make_test_command("orders", Uuid::new_v4());
            let dl = AngzarrDeadLetter::from_sequence_mismatch(
                &cmd,
                i,
                i + 5,
                MergeStrategy::MergeManual,
                &format!("agg-{}", i),
            );
            publisher.publish(dl).await.unwrap();
        }

        for i in 0..3 {
            let received = receiver.recv().await.expect("Should receive");
            assert_eq!(received.source_component, format!("agg-{}", i));
        }
    }

    #[test]
    fn test_channel_publisher_is_configured() {
        let (publisher, _receiver) = ChannelDeadLetterPublisher::new();
        assert!(publisher.is_configured());
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
        let config = DlqConfig {
            amqp_url: Some("amqp://localhost:5672".to_string()),
            ..Default::default()
        };
        assert!(config.is_configured());
    }

    #[test]
    fn test_dlq_config_kafka_configured() {
        let config = DlqConfig {
            kafka_brokers: Some("localhost:9092".to_string()),
            ..Default::default()
        };
        assert!(config.is_configured());
    }

    // ============================================================================
    // Publisher Factory Tests
    // ============================================================================

    #[test]
    fn test_create_publisher_default_is_noop() {
        let config = DlqConfig::default();
        let publisher = create_publisher(&config);
        assert!(!publisher.is_configured());
    }

    // ============================================================================
    // Error Tests
    // ============================================================================

    #[test]
    fn test_dlq_error_display() {
        let err = DlqError::NotConfigured;
        assert!(err.to_string().contains("not configured"));

        let err = DlqError::PublishFailed("connection refused".to_string());
        assert!(err.to_string().contains("connection refused"));

        let err = DlqError::InvalidDeadLetter("missing cover".to_string());
        assert!(err.to_string().contains("missing cover"));
    }
}
