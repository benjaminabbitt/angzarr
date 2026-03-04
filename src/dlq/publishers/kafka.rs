//! Kafka-based DLQ publisher.
//!
//! Publishes dead letters to topics named `angzarr-dlq-{domain}`.
//! Uses correlation_id as message key for ordering.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use prost::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use tracing::info;

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
            let kafka_config = config.kafka.clone();
            Box::pin(async move {
                if dlq_type != "kafka" {
                    return None;
                }
                let Some(kafka_config) = kafka_config else {
                    return Some(Err(DlqError::NotConfigured));
                };
                match KafkaDeadLetterPublisher::from_config(&kafka_config) {
                    Ok(publisher) => Some(Ok(Arc::new(publisher) as Arc<dyn DeadLetterPublisher>)),
                    Err(e) => Some(Err(e)),
                }
            })
        },
    }
}

/// Kafka-based DLQ publisher.
///
/// Publishes dead letters to topics named `angzarr-dlq-{domain}`.
/// Uses correlation_id as message key for ordering.
pub struct KafkaDeadLetterPublisher {
    producer: FutureProducer,
    topic_prefix: String,
}

impl KafkaDeadLetterPublisher {
    /// Create a new Kafka DLQ publisher.
    pub fn new(bootstrap_servers: &str) -> Result<Self, DlqError> {
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

    /// Create a new Kafka DLQ publisher from config.
    pub fn from_config(config: &super::super::config::KafkaDlqConfig) -> Result<Self, DlqError> {
        let mut client_config = ClientConfig::new();
        client_config
            .set("bootstrap.servers", &config.bootstrap_servers)
            .set("message.timeout.ms", "5000")
            .set("acks", "all")
            .set("enable.idempotence", "true");

        // Add SASL config if present
        if let Some(ref username) = config.sasl_username {
            client_config.set("sasl.username", username);
        }
        if let Some(ref password) = config.sasl_password {
            client_config.set("sasl.password", password);
        }
        if let Some(ref mechanism) = config.sasl_mechanism {
            client_config.set("sasl.mechanism", mechanism);
        }
        if let Some(ref protocol) = config.security_protocol {
            client_config.set("security.protocol", protocol);
        }

        let producer = client_config
            .create()
            .map_err(|e| DlqError::Connection(format!("Failed to create Kafka producer: {}", e)))?;

        info!(
            bootstrap_servers = %config.bootstrap_servers,
            topic_prefix = %config.topic_prefix,
            "Kafka DLQ publisher connected"
        );

        Ok(Self {
            producer,
            topic_prefix: config.topic_prefix.clone(),
        })
    }

    /// Build DLQ topic name for a domain.
    fn topic_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        format!("{}-{}", self.topic_prefix, sanitized)
    }
}

#[async_trait]
impl DeadLetterPublisher for KafkaDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        #[cfg(feature = "otel")]
        let start = std::time::Instant::now();

        let domain = dead_letter.domain().unwrap_or("unknown").to_string();
        let topic = self.topic_for_domain(&domain);
        #[cfg(feature = "otel")]
        let reason_type = dead_letter.reason_type();

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

        #[cfg(feature = "otel")]
        {
            use crate::advice::metrics::{
                backend_attr, domain_attr, reason_type_attr, DLQ_PUBLISH_DURATION,
                DLQ_PUBLISH_TOTAL,
            };
            DLQ_PUBLISH_DURATION.record(start.elapsed().as_secs_f64(), &[backend_attr("kafka")]);
            DLQ_PUBLISH_TOTAL.add(
                1,
                &[
                    domain_attr(&domain),
                    reason_type_attr(reason_type),
                    backend_attr("kafka"),
                ],
            );
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "kafka.test.rs"]
mod tests;
