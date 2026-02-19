//! Kafka sink for CloudEvents.
//!
//! Publishes CloudEvents to a Kafka topic. Messages are JSON-serialized
//! with the aggregate root ID as the message key for ordering.

use std::time::Duration;

use async_trait::async_trait;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use tracing::{debug, error, warn};

use cloudevents::event::AttributesReader;

use super::proto_encoding::encode_proto_single;
use super::sink::{CloudEventsSink, SinkError};
use super::types::{CloudEventEnvelope, ContentType};

/// Kafka sink configuration.
#[derive(Debug, Clone)]
pub struct KafkaSinkConfig {
    /// Kafka bootstrap servers (comma-separated).
    pub bootstrap_servers: String,

    /// Topic to publish CloudEvents to.
    pub topic: String,

    /// Message delivery timeout.
    pub timeout: Duration,

    /// SASL username (optional).
    pub sasl_username: Option<String>,

    /// SASL password (optional).
    pub sasl_password: Option<String>,

    /// SASL mechanism (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512).
    pub sasl_mechanism: Option<String>,

    /// Security protocol (PLAINTEXT, SSL, SASL_PLAINTEXT, SASL_SSL).
    pub security_protocol: Option<String>,

    /// SSL CA certificate path.
    pub ssl_ca_location: Option<String>,
}

impl Default for KafkaSinkConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: String::new(),
            topic: "cloudevents".to_string(),
            timeout: Duration::from_secs(5),
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: None,
            ssl_ca_location: None,
        }
    }
}

impl KafkaSinkConfig {
    /// Create config from environment variables.
    ///
    /// - `CLOUDEVENTS_KAFKA_BROKERS`: Required broker list
    /// - `CLOUDEVENTS_KAFKA_TOPIC`: Optional topic (default: "cloudevents")
    /// - `CLOUDEVENTS_KAFKA_TIMEOUT`: Optional timeout in seconds (default: 5)
    /// - `CLOUDEVENTS_KAFKA_SASL_USERNAME`: Optional SASL username
    /// - `CLOUDEVENTS_KAFKA_SASL_PASSWORD`: Optional SASL password
    /// - `CLOUDEVENTS_KAFKA_SASL_MECHANISM`: Optional SASL mechanism
    /// - `CLOUDEVENTS_KAFKA_SECURITY_PROTOCOL`: Optional security protocol
    /// - `CLOUDEVENTS_KAFKA_SSL_CA`: Optional SSL CA path
    pub fn from_env() -> Result<Self, SinkError> {
        let bootstrap_servers = std::env::var("CLOUDEVENTS_KAFKA_BROKERS")
            .map_err(|_| SinkError::Config("CLOUDEVENTS_KAFKA_BROKERS not set".to_string()))?;

        let topic =
            std::env::var("CLOUDEVENTS_KAFKA_TOPIC").unwrap_or_else(|_| "cloudevents".to_string());

        let timeout_secs = std::env::var("CLOUDEVENTS_KAFKA_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        Ok(Self {
            bootstrap_servers,
            topic,
            timeout: Duration::from_secs(timeout_secs),
            sasl_username: std::env::var("CLOUDEVENTS_KAFKA_SASL_USERNAME").ok(),
            sasl_password: std::env::var("CLOUDEVENTS_KAFKA_SASL_PASSWORD").ok(),
            sasl_mechanism: std::env::var("CLOUDEVENTS_KAFKA_SASL_MECHANISM").ok(),
            security_protocol: std::env::var("CLOUDEVENTS_KAFKA_SECURITY_PROTOCOL").ok(),
            ssl_ca_location: std::env::var("CLOUDEVENTS_KAFKA_SSL_CA").ok(),
        })
    }

    /// Set the bootstrap servers.
    pub fn with_bootstrap_servers(mut self, servers: String) -> Self {
        self.bootstrap_servers = servers;
        self
    }

    /// Set the topic.
    pub fn with_topic(mut self, topic: String) -> Self {
        self.topic = topic;
        self
    }

    /// Add SASL authentication.
    pub fn with_sasl(mut self, username: String, password: String, mechanism: String) -> Self {
        self.sasl_username = Some(username);
        self.sasl_password = Some(password);
        self.sasl_mechanism = Some(mechanism);
        self.security_protocol = Some("SASL_SSL".to_string());
        self
    }

    /// Build the Kafka ClientConfig.
    fn build_client_config(&self) -> ClientConfig {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &self.bootstrap_servers);
        config.set("message.timeout.ms", self.timeout.as_millis().to_string());
        config.set("acks", "all");
        config.set("enable.idempotence", "true");

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

        config
    }
}

/// Kafka sink for CloudEvents.
///
/// Publishes CloudEvents to a Kafka topic with JSON serialization.
/// Message keys are derived from the `subject` field (aggregate root ID).
pub struct KafkaSink {
    producer: FutureProducer,
    config: KafkaSinkConfig,
}

impl KafkaSink {
    /// Create a new Kafka sink with the given configuration.
    pub fn new(config: KafkaSinkConfig) -> Result<Self, SinkError> {
        if config.bootstrap_servers.is_empty() {
            return Err(SinkError::Config(
                "Kafka bootstrap servers not configured".to_string(),
            ));
        }

        let producer: FutureProducer = config
            .build_client_config()
            .create()
            .map_err(|e| SinkError::Kafka(format!("Failed to create Kafka producer: {}", e)))?;

        tracing::info!(
            bootstrap_servers = %config.bootstrap_servers,
            topic = %config.topic,
            "Kafka CloudEvents sink initialized"
        );

        Ok(Self { producer, config })
    }

    /// Create from environment variables.
    pub fn from_env() -> Result<Self, SinkError> {
        let config = KafkaSinkConfig::from_env()?;
        Self::new(config)
    }

    /// Publish a single event to Kafka.
    async fn publish_one(
        &self,
        event: &CloudEventEnvelope,
        format: ContentType,
    ) -> Result<(), SinkError> {
        let payload: Vec<u8> = match format {
            ContentType::Json => serde_json::to_string(event)?.into_bytes(),
            ContentType::Protobuf => encode_proto_single(event)?,
        };

        // Use subject (aggregate root ID) as message key for ordering
        let event_id = event.id();
        let key = event.subject().unwrap_or(event_id);

        let record = FutureRecord::to(&self.config.topic)
            .key(key)
            .payload(&payload);

        match self.producer.send(record, self.config.timeout).await {
            Ok((partition, offset)) => {
                debug!(
                    topic = %self.config.topic,
                    partition = partition,
                    offset = offset,
                    event_id = %event_id,
                    "CloudEvent published to Kafka"
                );
                Ok(())
            }
            Err((e, _)) => {
                error!(
                    topic = %self.config.topic,
                    event_id = %event_id,
                    error = %e,
                    "Failed to publish CloudEvent to Kafka"
                );
                Err(SinkError::Kafka(e.to_string()))
            }
        }
    }
}

#[async_trait]
impl CloudEventsSink for KafkaSink {
    async fn publish(
        &self,
        events: Vec<CloudEventEnvelope>,
        format: ContentType,
    ) -> Result<(), SinkError> {
        for event in &events {
            self.publish_one(event, format).await?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "kafka"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = KafkaSinkConfig::default();
        assert_eq!(config.topic, "cloudevents");
        assert_eq!(config.timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_config_builder() {
        let config = KafkaSinkConfig::default()
            .with_bootstrap_servers("localhost:9092".to_string())
            .with_topic("my-events".to_string())
            .with_sasl("user".to_string(), "pass".to_string(), "PLAIN".to_string());

        assert_eq!(config.bootstrap_servers, "localhost:9092");
        assert_eq!(config.topic, "my-events");
        assert_eq!(config.sasl_username, Some("user".to_string()));
        assert_eq!(config.sasl_mechanism, Some("PLAIN".to_string()));
        assert_eq!(config.security_protocol, Some("SASL_SSL".to_string()));
    }

    #[test]
    fn test_empty_bootstrap_servers_fails() {
        let config = KafkaSinkConfig::default();
        let result = KafkaSink::new(config);
        assert!(result.is_err());
    }
}
