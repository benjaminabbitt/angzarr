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
pub mod channel;
#[cfg(unix)]
pub mod ipc;
#[cfg(feature = "kafka")]
pub mod kafka;
#[cfg(feature = "lossy")]
pub mod lossy;
pub mod mock;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod outbox;

// Re-exports
#[cfg(feature = "amqp")]
pub use amqp::{AmqpConfig, AmqpEventBus};
pub use channel::{ChannelConfig, ChannelEventBus};
#[cfg(unix)]
pub use ipc::{
    IpcBroker, IpcBrokerConfig, IpcConfig, IpcEventBus, SubscriberInfo, SUBSCRIBERS_ENV_VAR,
};
#[cfg(feature = "kafka")]
pub use kafka::{KafkaEventBus, KafkaEventBusConfig};
#[cfg(feature = "lossy")]
pub use lossy::{LossyConfig, LossyEventBus, LossyStats};
pub use mock::MockEventBus;
#[cfg(feature = "postgres")]
pub use outbox::{OutboxConfig, PostgresOutboxEventBus, RecoveryTaskHandle};
#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
pub use outbox::{OutboxConfig, RecoveryTaskHandle, SqliteOutboxEventBus};

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

// ============================================================================
// Subscription Matching
// ============================================================================

use crate::proto::Target;
use crate::proto_ext::CoverExt;

/// Check if an EventBook matches a target filter.
///
/// A target matches if:
/// - The domain matches the target's domain
/// - AND either:
///   - The target has no types (matches all events from domain)
///   - OR at least one event in the book has a type_url ending with a target type
///
/// # Example
/// ```ignore
/// let target = Target {
///     domain: "order".to_string(),
///     types: vec!["OrderCreated".to_string(), "OrderShipped".to_string()],
/// };
/// if target_matches(&book, &target) {
///     // Process the event
/// }
/// ```
pub fn target_matches(book: &EventBook, target: &Target) -> bool {
    let routing_key = book.routing_key();

    // Routing key must match target domain (edition-prefixed)
    if target.domain != routing_key {
        return false;
    }

    // If no types specified, match all events from this domain
    if target.types.is_empty() {
        return true;
    }

    // Check if any event matches any target type
    book.pages.iter().any(|page| {
        page.event.as_ref().is_some_and(|event| {
            target.types.iter().any(|t| event.type_url.ends_with(t))
        })
    })
}

/// Check if an EventBook matches any of the given targets.
///
/// Returns true if at least one target matches the event book.
pub fn any_target_matches(book: &EventBook, targets: &[Target]) -> bool {
    targets.iter().any(|t| target_matches(book, t))
}

/// Interface for event delivery to projectors/sagas.
///
/// Implementations handle both publishing and subscriber creation through a
/// single interface. The runtime creates subscribers via `create_subscriber`
/// — no transport-specific code needed.
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

    /// Start consuming events (for bus implementations that require explicit start).
    ///
    /// Most implementations (AMQP, Kafka) start consuming automatically after subscribe.
    /// IPC requires explicit start because it spawns a blocking reader thread.
    ///
    /// Default implementation is a no-op for backwards compatibility.
    async fn start_consuming(&self) -> Result<()> {
        Ok(())
    }

    /// Create a subscriber bus that shares this bus's underlying transport.
    ///
    /// Events published on this bus will be delivered to the returned subscriber.
    /// Each implementation creates a transport-appropriate subscriber:
    /// - Channel: shares the broadcast channel with domain filtering
    /// - IPC: creates a named pipe subscriber
    /// - AMQP: creates a queue bound to the exchange
    /// - Kafka: creates a consumer group subscription
    ///
    /// # Arguments
    /// * `name` — subscriber identity (queue name, consumer group, pipe name)
    /// * `domain_filter` — restrict delivery to this domain (`None` = all domains)
    async fn create_subscriber(
        &self,
        name: &str,
        domain_filter: Option<&str>,
    ) -> Result<Arc<dyn EventBus>>;
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
    /// Kafka messaging.
    Kafka,
    /// In-memory channel (single process only).
    Channel,
    /// IPC via UDS/pipes (multi-process embedded mode, Unix only).
    #[cfg(unix)]
    Ipc,
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
    /// IPC-specific configuration (for embedded mode).
    #[cfg(unix)]
    pub ipc: IpcBusConfig,
    /// Outbox pattern configuration for guaranteed delivery.
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    pub outbox: outbox::OutboxConfig,
}

impl Default for MessagingConfig {
    fn default() -> Self {
        Self {
            messaging_type: MessagingType::Amqp,
            amqp: AmqpBusConfig::default(),
            kafka: KafkaConfig::default(),
            #[cfg(unix)]
            ipc: IpcBusConfig::default(),
            #[cfg(any(feature = "postgres", feature = "sqlite"))]
            outbox: outbox::OutboxConfig::default(),
        }
    }
}

/// IPC-specific configuration (for embedded mode).
#[cfg(unix)]
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct IpcBusConfig {
    /// Base path for pipes.
    pub base_path: String,
    /// Subscriber name (for subscriber mode).
    pub subscriber_name: Option<String>,
    /// Single domain to subscribe to (simpler env var).
    pub domain: Option<String>,
    /// Domains to subscribe to (for subscriber mode) - comma-separated when set via env var.
    pub domains: Option<Vec<String>>,
}

#[cfg(unix)]
impl IpcBusConfig {
    /// Get domains as a Vec, preferring `domains` over `domain`.
    pub fn get_domains(&self) -> Vec<String> {
        self.domains
            .clone()
            .or_else(|| {
                self.domain.as_ref().map(|d| {
                    // Support comma-separated domains in the single domain field
                    d.split(',').map(|s| s.trim().to_string()).collect()
                })
            })
            .unwrap_or_default()
    }
}

#[cfg(unix)]
impl Default for IpcBusConfig {
    fn default() -> Self {
        Self {
            base_path: "/tmp/angzarr".to_string(),
            subscriber_name: None,
            domain: None,
            domains: None,
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
/// - Channel: Always available (in-memory, no external deps)
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
                        let mut cfg =
                            KafkaEventBusConfig::publisher(&config.kafka.bootstrap_servers)
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
                Err(
                    "Kafka support requires the 'kafka' feature. Rebuild with --features kafka"
                        .into(),
                )
            }
        }
        MessagingType::Channel => {
            let channel_config = match mode {
                EventBusMode::Publisher => ChannelConfig::publisher(),
                EventBusMode::Subscriber { domain, .. } => ChannelConfig::subscriber(domain),
                EventBusMode::SubscriberAll { .. } => ChannelConfig::subscriber_all(),
            };

            let bus = ChannelEventBus::new(channel_config);
            info!(messaging_type = "channel", "Event bus initialized");
            Ok(Arc::new(bus))
        }
        #[cfg(unix)]
        MessagingType::Ipc => {
            let ipc_config = match mode {
                EventBusMode::Publisher => IpcConfig::publisher(&config.ipc.base_path),
                EventBusMode::Subscriber { domain, .. } => {
                    let name = config
                        .ipc
                        .subscriber_name
                        .clone()
                        .unwrap_or_else(|| format!("subscriber-{}", domain));
                    IpcConfig::subscriber(&config.ipc.base_path, name, vec![domain])
                }
                EventBusMode::SubscriberAll { queue } => {
                    let name = config.ipc.subscriber_name.clone().unwrap_or(queue);
                    let domains = config.ipc.get_domains();
                    IpcConfig::subscriber(&config.ipc.base_path, name, domains)
                }
            };

            let bus = IpcEventBus::new(ipc_config);
            info!(messaging_type = "ipc", "Event bus initialized");
            Ok(Arc::new(bus))
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
fn apply_kafka_security(
    mut cfg: KafkaEventBusConfig,
    kafka_cfg: &KafkaConfig,
) -> KafkaEventBusConfig {
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
mod tests;
