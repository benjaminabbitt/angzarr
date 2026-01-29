//! Kafka event bus implementation.
//!
//! Uses topics per domain for routing events to consumers.
//! Topic naming: `{topic_prefix}.events.{domain}`
//! Message key: aggregate root ID (ensures ordering per aggregate)

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use prost::Message;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::{BusError, EventBus, EventHandler, PublishResult, Result};
use crate::proto::EventBook;
use crate::proto_ext::CoverExt;

/// Configuration for Kafka connection.
#[derive(Clone, Debug)]
pub struct KafkaEventBusConfig {
    /// Kafka bootstrap servers (comma-separated).
    pub bootstrap_servers: String,
    /// Topic prefix for events (default: "angzarr").
    pub topic_prefix: String,
    /// Consumer group ID (required for subscribing).
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

impl KafkaEventBusConfig {
    /// Create config for publishing only.
    pub fn publisher(bootstrap_servers: impl Into<String>) -> Self {
        Self {
            bootstrap_servers: bootstrap_servers.into(),
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

    /// Create config for subscribing to specific domains.
    pub fn subscriber(
        bootstrap_servers: impl Into<String>,
        group_id: impl Into<String>,
        domains: Vec<String>,
    ) -> Self {
        Self {
            bootstrap_servers: bootstrap_servers.into(),
            topic_prefix: "angzarr".to_string(),
            group_id: Some(group_id.into()),
            domains: Some(domains),
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: None,
            ssl_ca_location: None,
        }
    }

    /// Create config for subscribing to all domains.
    pub fn subscriber_all(
        bootstrap_servers: impl Into<String>,
        group_id: impl Into<String>,
    ) -> Self {
        Self {
            bootstrap_servers: bootstrap_servers.into(),
            topic_prefix: "angzarr".to_string(),
            group_id: Some(group_id.into()),
            domains: None, // None means subscribe to all
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: None,
            ssl_ca_location: None,
        }
    }

    /// Add SASL authentication.
    pub fn with_sasl(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
        mechanism: impl Into<String>,
    ) -> Self {
        self.sasl_username = Some(username.into());
        self.sasl_password = Some(password.into());
        self.sasl_mechanism = Some(mechanism.into());
        self.security_protocol = Some("SASL_SSL".to_string());
        self
    }

    /// Set security protocol.
    pub fn with_security_protocol(mut self, protocol: impl Into<String>) -> Self {
        self.security_protocol = Some(protocol.into());
        self
    }

    /// Set SSL CA certificate location.
    pub fn with_ssl_ca(mut self, ca_location: impl Into<String>) -> Self {
        self.ssl_ca_location = Some(ca_location.into());
        self
    }

    /// Set topic prefix.
    pub fn with_topic_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.topic_prefix = prefix.into();
        self
    }

    /// Build the topic name for a domain.
    pub fn topic_for_domain(&self, domain: &str) -> String {
        format!("{}.events.{}", self.topic_prefix, domain)
    }

    /// Build a ClientConfig for producers.
    fn build_producer_config(&self) -> ClientConfig {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &self.bootstrap_servers);
        config.set("message.timeout.ms", "5000");
        config.set("acks", "all");
        config.set("enable.idempotence", "true");

        self.apply_security_config(&mut config);
        config
    }

    /// Build a ClientConfig for consumers.
    fn build_consumer_config(&self) -> ClientConfig {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &self.bootstrap_servers);
        config.set("enable.auto.commit", "false");
        config.set("auto.offset.reset", "earliest");

        if let Some(ref group_id) = self.group_id {
            config.set("group.id", group_id);
        }

        self.apply_security_config(&mut config);
        config
    }

    /// Apply security settings to a ClientConfig.
    fn apply_security_config(&self, config: &mut ClientConfig) {
        if let Some(ref protocol) = self.security_protocol {
            config.set("security.protocol", protocol);
        }

        if let Some(ref mechanism) = self.sasl_mechanism {
            config.set("sasl.mechanism", mechanism);
        }

        if let Some(ref username) = self.sasl_username {
            config.set("sasl.username", username);
        }

        if let Some(ref password) = self.sasl_password {
            config.set("sasl.password", password);
        }

        if let Some(ref ca_location) = self.ssl_ca_location {
            config.set("ssl.ca.location", ca_location);
        }
    }
}

/// Kafka event bus implementation.
///
/// Events are published to topics named `{topic_prefix}.events.{domain}`.
/// Message keys are the hex-encoded aggregate root ID for ordering guarantees.
/// Subscribers use consumer groups for load balancing across instances.
pub struct KafkaEventBus {
    producer: FutureProducer,
    config: KafkaEventBusConfig,
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    consumer: Option<Arc<StreamConsumer>>,
}

impl KafkaEventBus {
    /// Create a new Kafka event bus.
    pub async fn new(config: KafkaEventBusConfig) -> Result<Self> {
        let producer: FutureProducer = config
            .build_producer_config()
            .create()
            .map_err(|e| BusError::Connection(format!("Failed to create Kafka producer: {}", e)))?;

        info!(
            bootstrap_servers = %config.bootstrap_servers,
            topic_prefix = %config.topic_prefix,
            "Connected to Kafka"
        );

        // Create consumer if group_id is configured
        let consumer = if config.group_id.is_some() {
            let consumer: StreamConsumer =
                config.build_consumer_config().create().map_err(|e| {
                    BusError::Connection(format!("Failed to create Kafka consumer: {}", e))
                })?;
            Some(Arc::new(consumer))
        } else {
            None
        };

        Ok(Self {
            producer,
            config,
            handlers: Arc::new(RwLock::new(Vec::new())),
            consumer,
        })
    }

    /// Start consuming messages (call after subscribe).
    pub async fn start_consuming(&self) -> Result<()> {
        let consumer = self
            .consumer
            .as_ref()
            .ok_or_else(|| {
                BusError::Subscribe(
                    "No consumer configured. Use KafkaEventBusConfig::subscriber()".to_string(),
                )
            })?
            .clone();

        // Subscribe to topics
        let topics: Vec<String> = match &self.config.domains {
            Some(domains) => domains
                .iter()
                .map(|d| self.config.topic_for_domain(d))
                .collect(),
            None => {
                // Subscribe to all - use regex pattern
                // Note: This requires topic auto-creation or manual topic listing
                warn!("Subscribing to all domains requires topics to exist. Consider specifying domains.");
                vec![format!("{}.*", self.config.topic_prefix)]
            }
        };

        let topic_refs: Vec<&str> = topics.iter().map(|s| s.as_str()).collect();
        consumer
            .subscribe(&topic_refs)
            .map_err(|e| BusError::Subscribe(format!("Failed to subscribe to topics: {}", e)))?;

        info!(topics = ?topics, "Subscribed to Kafka topics");

        let handlers = self.handlers.clone();

        // Spawn consumer task
        tokio::spawn(async move {
            use futures::StreamExt;
            use rdkafka::message::Message as KafkaMessage;

            let mut stream = consumer.stream();

            while let Some(result) = stream.next().await {
                match result {
                    Ok(message) => {
                        let payload = match message.payload() {
                            Some(p) => p,
                            None => {
                                warn!("Received message with no payload");
                                continue;
                            }
                        };

                        match EventBook::decode(payload) {
                            Ok(book) => {
                                debug!(
                                    topic = %message.topic(),
                                    partition = message.partition(),
                                    offset = message.offset(),
                                    "Received event book"
                                );

                                let book = Arc::new(book);
                                let handlers_guard = handlers.read().await;

                                for handler in handlers_guard.iter() {
                                    if let Err(e) = handler.handle(Arc::clone(&book)).await {
                                        error!(error = %e, "Handler failed");
                                    }
                                }

                                // Commit offset after successful processing
                                if let Err(e) = consumer
                                    .commit_message(&message, rdkafka::consumer::CommitMode::Async)
                                {
                                    error!(error = %e, "Failed to commit offset");
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "Failed to decode event book");
                                // Still commit to avoid reprocessing malformed messages
                                let _ = consumer
                                    .commit_message(&message, rdkafka::consumer::CommitMode::Async);
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Kafka consumer error");
                    }
                }
            }
        });

        Ok(())
    }
}

#[async_trait]
impl EventBus for KafkaEventBus {
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        let domain = book
            .cover()
            .map(|c| c.domain.as_str())
            .ok_or_else(|| BusError::Publish("EventBook missing cover/domain".to_string()))?;

        let topic = self.config.topic_for_domain(domain);
        let key = book.root_id_hex();
        let payload = book.encode_to_vec();

        let mut record = FutureRecord::to(&topic).payload(&payload);

        if let Some(ref k) = key {
            record = record.key(k);
        }

        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| BusError::Publish(format!("Failed to publish: {}", e)))?;

        debug!(
            topic = %topic,
            key = ?key,
            "Published event book to Kafka"
        );

        // Kafka is async-only, no synchronous projections
        Ok(PublishResult::default())
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        if self.consumer.is_none() {
            return Err(BusError::Subscribe(
                "Cannot subscribe: no consumer configured. Use KafkaEventBusConfig::subscriber()"
                    .to_string(),
            ));
        }

        let mut handlers = self.handlers.write().await;
        handlers.push(handler);

        Ok(())
    }
}

#[cfg(test)]
mod tests;
