//! Event bus for async delivery.
//!
//! This module contains:
//! - `EventBus` trait: Event delivery to projectors/sagas
//! - `EventHandler` trait: For processing events
//! - Bus configuration types
//! - Implementations: AMQP (RabbitMQ), Kafka, Mock

use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use serde::Deserialize;
use tonic::Status;
use tracing::info;

use crate::proto::{EventBook, Projection};

// Implementation modules
#[cfg(feature = "amqp")]
pub mod amqp;
#[cfg(feature = "kafka")]
pub mod kafka;
pub mod mock;

// Re-exports
#[cfg(feature = "amqp")]
pub use amqp::{AmqpConfig, AmqpEventBus};
#[cfg(feature = "kafka")]
pub use kafka::{KafkaEventBus, KafkaEventBusConfig};
pub use mock::MockEventBus;

// ============================================================================
// Traits
// ============================================================================

/// Result type for bus operations.
pub type Result<T> = std::result::Result<T, BusError>;

/// Errors that can occur during bus operations.
#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Publish failed: {0}")]
    Publish(String),

    #[error("Subscribe failed: {0}")]
    Subscribe(String),

    #[error("Projector '{name}' failed: {message}")]
    ProjectorFailed { name: String, message: String },

    #[error("Saga '{name}' failed: {message}")]
    SagaFailed { name: String, message: String },

    #[error("gRPC error: {0}")]
    Grpc(#[from] Status),

    #[error("Subscribe not supported for this bus type")]
    SubscribeNotSupported,
}

/// Handler for processing events from the bus.
pub trait EventHandler: Send + Sync {
    /// Process an event book.
    fn handle(&self, book: Arc<EventBook>)
        -> BoxFuture<'static, std::result::Result<(), BusError>>;
}

/// Result of publishing events to the bus.
#[derive(Debug, Default)]
pub struct PublishResult {
    /// Projections returned by synchronous projectors.
    pub projections: Vec<Projection>,
}

/// Interface for event delivery to projectors/sagas.
///
/// Implementations:
/// - `AmqpEventBus`: RabbitMQ via AMQP
/// - `MockEventBus`: In-memory mock for testing
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish events to consumers.
    ///
    /// The EventBook is wrapped in Arc to enforce immutability during distribution.
    /// All consumers receive a zero-copy reference to the same immutable data.
    ///
    /// For synchronous events, this blocks until all consumers acknowledge.
    /// For async events, this returns immediately after queuing.
    ///
    /// Returns projections from synchronous projectors.
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult>;

    /// Subscribe to events (for projector/saga implementations).
    ///
    /// The handler will be called for each event book received.
    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()>;
}

// ============================================================================
// Configuration
// ============================================================================

/// Messaging type discriminator.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessagingType {
    /// AMQP/RabbitMQ messaging.
    #[default]
    Amqp,
    /// Kafka messaging (future).
    Kafka,
}

/// Messaging configuration (discriminated union).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MessagingConfig {
    /// Messaging type discriminator.
    #[serde(rename = "type")]
    pub messaging_type: MessagingType,
    /// AMQP-specific configuration.
    pub amqp: AmqpBusConfig,
    /// Kafka-specific configuration (future).
    pub kafka: KafkaConfig,
}

impl Default for MessagingConfig {
    fn default() -> Self {
        Self {
            messaging_type: MessagingType::Amqp,
            amqp: AmqpBusConfig::default(),
            kafka: KafkaConfig::default(),
        }
    }
}

/// AMQP-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AmqpBusConfig {
    /// AMQP connection URL.
    pub url: String,
    /// Domain to subscribe to (for aggregate mode, this is the command queue).
    pub domain: Option<String>,
    /// Domains to subscribe to (for projector/saga modes).
    pub domains: Option<Vec<String>>,
}

impl Default for AmqpBusConfig {
    fn default() -> Self {
        Self {
            url: "amqp://localhost:5672".to_string(),
            domain: None,
            domains: None,
        }
    }
}

/// Kafka-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KafkaConfig {
    /// Kafka bootstrap servers (comma-separated).
    pub bootstrap_servers: String,
    /// Topic prefix for events.
    pub topic_prefix: String,
    /// Consumer group ID.
    pub group_id: Option<String>,
    /// Domains to subscribe to (for consumers).
    pub domains: Option<Vec<String>>,
    /// SASL username (optional, for authenticated clusters).
    pub sasl_username: Option<String>,
    /// SASL password (optional, for authenticated clusters).
    pub sasl_password: Option<String>,
    /// SASL mechanism (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512).
    pub sasl_mechanism: Option<String>,
    /// Security protocol (PLAINTEXT, SSL, SASL_PLAINTEXT, SASL_SSL).
    pub security_protocol: Option<String>,
    /// SSL CA certificate path (for SSL connections).
    pub ssl_ca_location: Option<String>,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            topic_prefix: "angzarr".to_string(),
            group_id: None,
            domains: None,
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: None,
            ssl_ca_location: None,
        }
    }
}

// ============================================================================
// Factory
// ============================================================================

/// Initialize event bus based on configuration.
///
/// Returns the appropriate EventBus implementation based on messaging_type.
/// Requires the corresponding feature to be enabled:
/// - AMQP: `--features amqp` (included in default)
/// - Kafka: `--features kafka`
pub async fn init_event_bus(
    config: &MessagingConfig,
    mode: EventBusMode,
) -> std::result::Result<Arc<dyn EventBus>, Box<dyn std::error::Error + Send + Sync>> {
    match config.messaging_type {
        MessagingType::Amqp => {
            #[cfg(feature = "amqp")]
            {
                let amqp_config = match mode {
                    EventBusMode::Publisher => AmqpConfig::publisher(&config.amqp.url),
                    EventBusMode::Subscriber { queue, domain } => {
                        AmqpConfig::subscriber(&config.amqp.url, queue, &domain)
                    }
                    EventBusMode::SubscriberAll { queue } => {
                        AmqpConfig::subscriber_all(&config.amqp.url, queue)
                    }
                };

                let bus = AmqpEventBus::new(amqp_config).await?;
                info!(messaging_type = "amqp", "Event bus initialized");
                Ok(Arc::new(bus))
            }

            #[cfg(not(feature = "amqp"))]
            {
                Err("AMQP support requires the 'amqp' feature. Rebuild with --features amqp".into())
            }
        }
        MessagingType::Kafka => {
            #[cfg(feature = "kafka")]
            {
                let kafka_config = match mode {
                    EventBusMode::Publisher => {
                        let mut cfg = KafkaEventBusConfig::publisher(&config.kafka.bootstrap_servers)
                            .with_topic_prefix(&config.kafka.topic_prefix);
                        cfg = apply_kafka_security(cfg, &config.kafka);
                        cfg
                    }
                    EventBusMode::Subscriber { queue, domain } => {
                        let mut cfg = KafkaEventBusConfig::subscriber(
                            &config.kafka.bootstrap_servers,
                            queue,
                            vec![domain],
                        )
                        .with_topic_prefix(&config.kafka.topic_prefix);
                        cfg = apply_kafka_security(cfg, &config.kafka);
                        cfg
                    }
                    EventBusMode::SubscriberAll { queue } => {
                        let domains = config.kafka.domains.clone().unwrap_or_default();
                        let mut cfg = if domains.is_empty() {
                            KafkaEventBusConfig::subscriber_all(
                                &config.kafka.bootstrap_servers,
                                queue,
                            )
                        } else {
                            KafkaEventBusConfig::subscriber(
                                &config.kafka.bootstrap_servers,
                                queue,
                                domains,
                            )
                        };
                        cfg = cfg.with_topic_prefix(&config.kafka.topic_prefix);
                        cfg = apply_kafka_security(cfg, &config.kafka);
                        cfg
                    }
                };

                let bus = KafkaEventBus::new(kafka_config).await?;
                info!(messaging_type = "kafka", "Event bus initialized");
                Ok(Arc::new(bus))
            }

            #[cfg(not(feature = "kafka"))]
            {
                Err("Kafka support requires the 'kafka' feature. Rebuild with --features kafka".into())
            }
        }
    }
}

/// Mode for event bus initialization.
#[derive(Debug, Clone)]
pub enum EventBusMode {
    /// Publisher-only mode (no consuming).
    Publisher,
    /// Subscriber mode for a specific domain.
    Subscriber {
        /// Queue/group name.
        queue: String,
        /// Domain to subscribe to.
        domain: String,
    },
    /// Subscriber mode for all domains.
    SubscriberAll {
        /// Queue/group name.
        queue: String,
    },
}

#[cfg(feature = "kafka")]
fn apply_kafka_security(mut cfg: KafkaEventBusConfig, kafka_cfg: &KafkaConfig) -> KafkaEventBusConfig {
    if let (Some(ref user), Some(ref pass), Some(ref mechanism)) = (
        &kafka_cfg.sasl_username,
        &kafka_cfg.sasl_password,
        &kafka_cfg.sasl_mechanism,
    ) {
        cfg = cfg.with_sasl(user, pass, mechanism);
    }

    if let Some(ref protocol) = kafka_cfg.security_protocol {
        cfg = cfg.with_security_protocol(protocol);
    }

    if let Some(ref ca) = kafka_cfg.ssl_ca_location {
        cfg = cfg.with_ssl_ca(ca);
    }

    cfg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_messaging_config_default() {
        let config = MessagingConfig::default();
        assert_eq!(config.messaging_type, MessagingType::Amqp);
        assert_eq!(config.amqp.url, "amqp://localhost:5672");
    }
}
