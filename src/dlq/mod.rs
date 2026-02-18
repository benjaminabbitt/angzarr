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

use crate::proto::{
    angzarr_dead_letter, AngzarrDeadLetter as ProtoAngzarrDeadLetter, CommandBook, Cover,
    EventBook, EventProcessingFailedDetails as ProtoEventProcessingFailedDetails, MergeStrategy,
    PayloadRetrievalFailedDetails as ProtoPayloadRetrievalFailedDetails, PayloadStorageType,
    SequenceMismatchDetails as ProtoSequenceMismatchDetails,
};

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
            occurred_at: self.occurred_at.clone(),
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

// ============================================================================
// AMQP Dead Letter Publisher
// ============================================================================

/// AMQP-based DLQ publisher using RabbitMQ.
///
/// Publishes dead letters to a topic exchange with routing key: `{domain}`.
/// Exchange name: `angzarr.dlq`
#[cfg(feature = "amqp")]
pub struct AmqpDeadLetterPublisher {
    pool: deadpool_lapin::Pool,
    exchange: String,
}

#[cfg(feature = "amqp")]
impl AmqpDeadLetterPublisher {
    /// DLQ exchange name.
    const DLQ_EXCHANGE: &'static str = "angzarr.dlq";

    /// Create a new AMQP DLQ publisher.
    pub async fn new(amqp_url: &str) -> Result<Self, DlqError> {
        use deadpool_lapin::{Manager, Pool};
        use lapin::{options::ExchangeDeclareOptions, types::FieldTable, ExchangeKind};

        let manager = Manager::new(amqp_url.to_string(), Default::default());
        let pool = Pool::builder(manager)
            .max_size(5)
            .build()
            .map_err(|e| DlqError::Connection(format!("Failed to create AMQP pool: {}", e)))?;

        // Verify connection and declare exchange
        let conn = pool
            .get()
            .await
            .map_err(|e| DlqError::Connection(format!("Failed to connect to AMQP: {}", e)))?;

        let channel = conn
            .create_channel()
            .await
            .map_err(|e| DlqError::Connection(format!("Failed to create channel: {}", e)))?;

        channel
            .exchange_declare(
                Self::DLQ_EXCHANGE,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| DlqError::Connection(format!("Failed to declare DLQ exchange: {}", e)))?;

        info!(exchange = %Self::DLQ_EXCHANGE, "AMQP DLQ publisher connected");

        Ok(Self {
            pool,
            exchange: Self::DLQ_EXCHANGE.to_string(),
        })
    }
}

#[cfg(feature = "amqp")]
#[async_trait]
impl DeadLetterPublisher for AmqpDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        use lapin::{options::BasicPublishOptions, BasicProperties};
        use prost::Message;

        let domain = dead_letter.domain().unwrap_or("unknown");
        let routing_key = domain.to_string();

        // Serialize to proto
        let proto = dead_letter.to_proto();
        let payload = proto.encode_to_vec();

        let conn = self
            .pool
            .get()
            .await
            .map_err(|e| DlqError::Connection(format!("Failed to get connection: {}", e)))?;

        let channel = conn
            .create_channel()
            .await
            .map_err(|e| DlqError::Connection(format!("Failed to create channel: {}", e)))?;

        let properties = BasicProperties::default()
            .with_content_type("application/protobuf".into())
            .with_delivery_mode(2); // persistent

        channel
            .basic_publish(
                &self.exchange,
                &routing_key,
                BasicPublishOptions::default(),
                &payload,
                properties,
            )
            .await
            .map_err(|e| DlqError::PublishFailed(format!("Failed to publish: {}", e)))?
            .await
            .map_err(|e| DlqError::PublishFailed(format!("Publish confirmation failed: {}", e)))?;

        info!(
            exchange = %self.exchange,
            routing_key = %routing_key,
            reason = %dead_letter.rejection_reason,
            "Published to AMQP DLQ"
        );

        Ok(())
    }
}

// ============================================================================
// Kafka Dead Letter Publisher
// ============================================================================

/// Kafka-based DLQ publisher.
///
/// Publishes dead letters to topics named `angzarr-dlq-{domain}`.
/// Uses correlation_id as message key for ordering.
#[cfg(feature = "kafka")]
pub struct KafkaDeadLetterPublisher {
    producer: rdkafka::producer::FutureProducer,
    topic_prefix: String,
}

#[cfg(feature = "kafka")]
impl KafkaDeadLetterPublisher {
    /// Create a new Kafka DLQ publisher.
    pub fn new(bootstrap_servers: &str) -> Result<Self, DlqError> {
        use rdkafka::ClientConfig;

        let producer = ClientConfig::new()
            .set("bootstrap.servers", bootstrap_servers)
            .set("message.timeout.ms", "5000")
            .set("acks", "all")
            .set("enable.idempotence", "true")
            .create()
            .map_err(|e| DlqError::Connection(format!("Failed to create Kafka producer: {}", e)))?;

        info!(bootstrap_servers = %bootstrap_servers, "Kafka DLQ publisher connected");

        Ok(Self {
            producer,
            topic_prefix: "angzarr-dlq".to_string(),
        })
    }

    /// Build DLQ topic name for a domain.
    fn topic_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        format!("{}-{}", self.topic_prefix, sanitized)
    }
}

#[cfg(feature = "kafka")]
#[async_trait]
impl DeadLetterPublisher for KafkaDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        use prost::Message;
        use rdkafka::producer::FutureRecord;
        use std::time::Duration;

        let domain = dead_letter.domain().unwrap_or("unknown");
        let topic = self.topic_for_domain(domain);

        // Use correlation_id as key for ordering
        let key = dead_letter
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        // Serialize to proto
        let proto = dead_letter.to_proto();
        let payload = proto.encode_to_vec();

        let record = FutureRecord::to(&topic).payload(&payload).key(&key);

        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| DlqError::PublishFailed(format!("Failed to publish: {}", e)))?;

        info!(
            topic = %topic,
            key = %key,
            reason = %dead_letter.rejection_reason,
            "Published to Kafka DLQ"
        );

        Ok(())
    }
}

// ============================================================================
// GCP Pub/Sub Dead Letter Publisher
// ============================================================================

/// GCP Pub/Sub-based DLQ publisher.
///
/// Publishes dead letters to topics named `angzarr-dlq-{domain}`.
#[cfg(feature = "pubsub")]
pub struct PubSubDeadLetterPublisher {
    client: google_cloud_pubsub::client::Client,
    topic_prefix: String,
    publishers: Arc<
        tokio::sync::RwLock<
            std::collections::HashMap<String, google_cloud_pubsub::publisher::Publisher>,
        >,
    >,
}

#[cfg(feature = "pubsub")]
impl PubSubDeadLetterPublisher {
    /// Create a new Pub/Sub DLQ publisher.
    pub async fn new() -> Result<Self, DlqError> {
        use google_cloud_pubsub::client::{Client, ClientConfig};

        let config = ClientConfig::default().with_auth().await.map_err(|e| {
            DlqError::Connection(format!("Failed to configure Pub/Sub auth: {}", e))
        })?;

        let client = Client::new(config)
            .await
            .map_err(|e| DlqError::Connection(format!("Failed to create Pub/Sub client: {}", e)))?;

        info!("Pub/Sub DLQ publisher connected");

        Ok(Self {
            client,
            topic_prefix: "angzarr-dlq".to_string(),
            publishers: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Build DLQ topic name for a domain.
    fn topic_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        format!("{}-{}", self.topic_prefix, sanitized)
    }

    /// Get or create a publisher for a topic.
    async fn get_publisher(
        &self,
        domain: &str,
    ) -> Result<google_cloud_pubsub::publisher::Publisher, DlqError> {
        let topic_name = self.topic_for_domain(domain);

        // Check cache
        {
            let publishers = self.publishers.read().await;
            if let Some(publisher) = publishers.get(&topic_name) {
                return Ok(publisher.clone());
            }
        }

        // Create topic if needed
        let topic = self.client.topic(&topic_name);
        if !topic.exists(None).await.map_err(|e| {
            DlqError::PublishFailed(format!("Failed to check topic existence: {}", e))
        })? {
            topic.create(None, None).await.map_err(|e| {
                DlqError::PublishFailed(format!("Failed to create topic {}: {}", topic_name, e))
            })?;
            info!(topic = %topic_name, "Created Pub/Sub DLQ topic");
        }

        let publisher = topic.new_publisher(None);

        // Cache it
        {
            let mut publishers = self.publishers.write().await;
            publishers.insert(topic_name.clone(), publisher.clone());
        }

        Ok(publisher)
    }
}

#[cfg(feature = "pubsub")]
#[async_trait]
impl DeadLetterPublisher for PubSubDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        use google_cloud_googleapis::pubsub::v1::PubsubMessage;
        use prost::Message;

        let domain = dead_letter.domain().unwrap_or("unknown");
        let publisher = self.get_publisher(domain).await?;

        // Serialize to proto
        let proto = dead_letter.to_proto();
        let payload = proto.encode_to_vec();

        // Build message with attributes
        let correlation_id = dead_letter
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        let mut attributes = std::collections::HashMap::new();
        attributes.insert("domain".to_string(), domain.to_string());
        attributes.insert("correlation_id".to_string(), correlation_id.clone());

        let message = PubsubMessage {
            data: payload.into(),
            ordering_key: correlation_id,
            attributes,
            ..Default::default()
        };

        let awaiter = publisher.publish(message).await;
        awaiter
            .get()
            .await
            .map_err(|e| DlqError::PublishFailed(format!("Failed to publish: {}", e)))?;

        info!(
            domain = %domain,
            reason = %dead_letter.rejection_reason,
            "Published to Pub/Sub DLQ"
        );

        Ok(())
    }
}

// ============================================================================
// AWS SNS/SQS Dead Letter Publisher
// ============================================================================

/// AWS SNS-based DLQ publisher.
///
/// Publishes dead letters to SNS topics named `angzarr-dlq-{domain}`.
#[cfg(feature = "sns-sqs")]
pub struct SnsSqsDeadLetterPublisher {
    sns: aws_sdk_sns::Client,
    topic_prefix: String,
    topic_arns: Arc<tokio::sync::RwLock<std::collections::HashMap<String, String>>>,
}

#[cfg(feature = "sns-sqs")]
impl SnsSqsDeadLetterPublisher {
    /// Create a new SNS/SQS DLQ publisher.
    pub async fn new(region: Option<&str>, endpoint_url: Option<&str>) -> Result<Self, DlqError> {
        use aws_config::BehaviorVersion;

        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());

        if let Some(region) = region {
            config_builder = config_builder.region(aws_config::Region::new(region.to_string()));
        }

        if let Some(endpoint) = endpoint_url {
            config_builder = config_builder.endpoint_url(endpoint);
        }

        let aws_config = config_builder.load().await;
        let sns = aws_sdk_sns::Client::new(&aws_config);

        info!(
            region = ?region,
            endpoint = ?endpoint_url,
            "SNS/SQS DLQ publisher connected"
        );

        Ok(Self {
            sns,
            topic_prefix: "angzarr-dlq".to_string(),
            topic_arns: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Build DLQ topic name for a domain.
    fn topic_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        format!("{}-{}", self.topic_prefix, sanitized)
    }

    /// Get or create an SNS topic ARN for a domain.
    async fn get_or_create_topic(&self, domain: &str) -> Result<String, DlqError> {
        let topic_name = self.topic_for_domain(domain);

        // Check cache
        {
            let arns = self.topic_arns.read().await;
            if let Some(arn) = arns.get(&topic_name) {
                return Ok(arn.clone());
            }
        }

        // Create topic (idempotent)
        let result = self
            .sns
            .create_topic()
            .name(&topic_name)
            .send()
            .await
            .map_err(|e| DlqError::PublishFailed(format!("Failed to create SNS topic: {}", e)))?;

        let arn = result
            .topic_arn()
            .ok_or_else(|| DlqError::PublishFailed("SNS create_topic returned no ARN".to_string()))?
            .to_string();

        // Cache it
        {
            let mut arns = self.topic_arns.write().await;
            arns.insert(topic_name.clone(), arn.clone());
        }

        info!(topic = %topic_name, arn = %arn, "Created/found SNS DLQ topic");
        Ok(arn)
    }
}

#[cfg(feature = "sns-sqs")]
#[async_trait]
impl DeadLetterPublisher for SnsSqsDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        use aws_sdk_sns::types::MessageAttributeValue;
        use base64::prelude::*;
        use prost::Message;

        let domain = dead_letter.domain().unwrap_or("unknown");
        let topic_arn = self.get_or_create_topic(domain).await?;

        // Serialize to proto, then base64 encode
        let proto = dead_letter.to_proto();
        let payload = proto.encode_to_vec();
        let message = BASE64_STANDARD.encode(&payload);

        // Build message attributes
        let correlation_id = dead_letter
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        let mut attrs = std::collections::HashMap::new();
        attrs.insert(
            "domain".to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(domain)
                .build()
                .map_err(|e| {
                    DlqError::PublishFailed(format!("Failed to build attribute: {}", e))
                })?,
        );
        attrs.insert(
            "correlation_id".to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&correlation_id)
                .build()
                .map_err(|e| {
                    DlqError::PublishFailed(format!("Failed to build attribute: {}", e))
                })?,
        );

        self.sns
            .publish()
            .topic_arn(&topic_arn)
            .message(&message)
            .set_message_attributes(Some(attrs))
            .send()
            .await
            .map_err(|e| DlqError::PublishFailed(format!("Failed to publish to SNS: {}", e)))?;

        info!(
            topic_arn = %topic_arn,
            domain = %domain,
            reason = %dead_letter.rejection_reason,
            "Published to SNS DLQ"
        );

        Ok(())
    }
}

/// DLQ backend selection.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DlqBackend {
    /// No DLQ (logs only).
    #[default]
    None,
    /// In-memory channel (for standalone mode and testing).
    Channel,
    /// RabbitMQ/AMQP.
    Amqp,
    /// Apache Kafka.
    Kafka,
    /// Google Cloud Pub/Sub.
    PubSub,
    /// AWS SNS/SQS.
    SnsSqs,
}

/// Configuration for DLQ publishers.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct DlqConfig {
    /// Selected backend.
    pub backend: DlqBackend,
    /// AMQP connection URL (for AMQP backend).
    pub amqp_url: Option<String>,
    /// Kafka bootstrap servers (for Kafka backend).
    pub kafka_brokers: Option<String>,
    /// AWS region (for SNS/SQS backend).
    pub aws_region: Option<String>,
    /// AWS endpoint URL (for LocalStack or testing).
    pub aws_endpoint_url: Option<String>,
}

impl DlqConfig {
    /// Create config for channel backend (standalone mode).
    pub fn channel() -> Self {
        Self {
            backend: DlqBackend::Channel,
            ..Default::default()
        }
    }

    /// Create config for AMQP backend.
    pub fn amqp(url: impl Into<String>) -> Self {
        Self {
            backend: DlqBackend::Amqp,
            amqp_url: Some(url.into()),
            ..Default::default()
        }
    }

    /// Create config for Kafka backend.
    pub fn kafka(brokers: impl Into<String>) -> Self {
        Self {
            backend: DlqBackend::Kafka,
            kafka_brokers: Some(brokers.into()),
            ..Default::default()
        }
    }

    /// Create config for Pub/Sub backend.
    pub fn pubsub() -> Self {
        Self {
            backend: DlqBackend::PubSub,
            ..Default::default()
        }
    }

    /// Create config for SNS/SQS backend.
    pub fn sns_sqs() -> Self {
        Self {
            backend: DlqBackend::SnsSqs,
            ..Default::default()
        }
    }

    /// Set AWS region (for SNS/SQS).
    pub fn with_aws_region(mut self, region: impl Into<String>) -> Self {
        self.aws_region = Some(region.into());
        self
    }

    /// Set AWS endpoint URL (for LocalStack).
    pub fn with_aws_endpoint(mut self, url: impl Into<String>) -> Self {
        self.aws_endpoint_url = Some(url.into());
        self
    }

    /// Check if any DLQ backend is configured.
    pub fn is_configured(&self) -> bool {
        self.backend != DlqBackend::None
    }
}

/// Create a DLQ publisher based on configuration.
///
/// Returns NoopDeadLetterPublisher if nothing is configured.
/// For async backends (AMQP, Kafka, PubSub, SNS/SQS), use `create_publisher_async`.
pub fn create_publisher(config: &DlqConfig) -> Arc<dyn DeadLetterPublisher> {
    match config.backend {
        DlqBackend::None => {
            debug!("No DLQ configured, using noop publisher");
            Arc::new(NoopDeadLetterPublisher)
        }
        DlqBackend::Channel => {
            // For channel, caller should use ChannelDeadLetterPublisher::new() directly
            // to get the receiver end. Return noop as fallback.
            warn!("Channel DLQ requires manual setup with ChannelDeadLetterPublisher::new()");
            Arc::new(NoopDeadLetterPublisher)
        }
        _ => {
            // Async backends require create_publisher_async
            warn!(
                backend = ?config.backend,
                "DLQ backend requires async initialization, use create_publisher_async()"
            );
            Arc::new(NoopDeadLetterPublisher)
        }
    }
}

/// Create a DLQ publisher asynchronously.
///
/// Required for backends that need async initialization (AMQP, Kafka, PubSub, SNS/SQS).
pub async fn create_publisher_async(
    config: &DlqConfig,
) -> Result<Arc<dyn DeadLetterPublisher>, DlqError> {
    match config.backend {
        DlqBackend::None => {
            debug!("No DLQ configured, using noop publisher");
            Ok(Arc::new(NoopDeadLetterPublisher))
        }
        DlqBackend::Channel => {
            // Caller should use ChannelDeadLetterPublisher::new() directly
            warn!("Channel DLQ requires manual setup with ChannelDeadLetterPublisher::new()");
            Ok(Arc::new(NoopDeadLetterPublisher))
        }
        #[cfg(feature = "amqp")]
        DlqBackend::Amqp => {
            let url = config
                .amqp_url
                .as_ref()
                .ok_or_else(|| DlqError::NotConfigured)?;
            let publisher = AmqpDeadLetterPublisher::new(url).await?;
            Ok(Arc::new(publisher))
        }
        #[cfg(not(feature = "amqp"))]
        DlqBackend::Amqp => Err(DlqError::NotConfigured),
        #[cfg(feature = "kafka")]
        DlqBackend::Kafka => {
            let brokers = config
                .kafka_brokers
                .as_ref()
                .ok_or_else(|| DlqError::NotConfigured)?;
            let publisher = KafkaDeadLetterPublisher::new(brokers)?;
            Ok(Arc::new(publisher))
        }
        #[cfg(not(feature = "kafka"))]
        DlqBackend::Kafka => Err(DlqError::NotConfigured),
        #[cfg(feature = "pubsub")]
        DlqBackend::PubSub => {
            let publisher = PubSubDeadLetterPublisher::new().await?;
            Ok(Arc::new(publisher))
        }
        #[cfg(not(feature = "pubsub"))]
        DlqBackend::PubSub => Err(DlqError::NotConfigured),
        #[cfg(feature = "sns-sqs")]
        DlqBackend::SnsSqs => {
            let publisher = SnsSqsDeadLetterPublisher::new(
                config.aws_region.as_deref(),
                config.aws_endpoint_url.as_deref(),
            )
            .await?;
            Ok(Arc::new(publisher))
        }
        #[cfg(not(feature = "sns-sqs"))]
        DlqBackend::SnsSqs => Err(DlqError::NotConfigured),
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
        let config = DlqConfig::amqp("amqp://localhost:5672");
        assert!(config.is_configured());
        assert_eq!(config.backend, DlqBackend::Amqp);
        assert_eq!(config.amqp_url, Some("amqp://localhost:5672".to_string()));
    }

    #[test]
    fn test_dlq_config_kafka_configured() {
        let config = DlqConfig::kafka("localhost:9092");
        assert!(config.is_configured());
        assert_eq!(config.backend, DlqBackend::Kafka);
        assert_eq!(config.kafka_brokers, Some("localhost:9092".to_string()));
    }

    #[test]
    fn test_dlq_config_channel_configured() {
        let config = DlqConfig::channel();
        assert!(config.is_configured());
        assert_eq!(config.backend, DlqBackend::Channel);
    }

    #[test]
    fn test_dlq_config_pubsub_configured() {
        let config = DlqConfig::pubsub();
        assert!(config.is_configured());
        assert_eq!(config.backend, DlqBackend::PubSub);
    }

    #[test]
    fn test_dlq_config_sns_sqs_configured() {
        let config = DlqConfig::sns_sqs()
            .with_aws_region("us-east-1")
            .with_aws_endpoint("http://localhost:4566");
        assert!(config.is_configured());
        assert_eq!(config.backend, DlqBackend::SnsSqs);
        assert_eq!(config.aws_region, Some("us-east-1".to_string()));
        assert_eq!(
            config.aws_endpoint_url,
            Some("http://localhost:4566".to_string())
        );
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
