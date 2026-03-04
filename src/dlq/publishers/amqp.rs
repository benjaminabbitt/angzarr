//! AMQP-based DLQ publisher using RabbitMQ.
//!
//! Publishes dead letters to a topic exchange with routing key: `{domain}`.
//! Exchange name: `angzarr.dlq`

use std::sync::Arc;

use async_trait::async_trait;
use deadpool_lapin::{Manager, Pool};
use lapin::{
    options::BasicPublishOptions, options::ExchangeDeclareOptions, types::FieldTable,
    BasicProperties, ExchangeKind,
};
use prost::Message;
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
            let amqp_config = config.amqp.clone();
            Box::pin(async move {
                if dlq_type != "amqp" {
                    return None;
                }
                let Some(amqp_config) = amqp_config else {
                    return Some(Err(DlqError::NotConfigured));
                };
                match AmqpDeadLetterPublisher::new(&amqp_config.url).await {
                    Ok(publisher) => Some(Ok(Arc::new(publisher) as Arc<dyn DeadLetterPublisher>)),
                    Err(e) => Some(Err(e)),
                }
            })
        },
    }
}

/// AMQP-based DLQ publisher using RabbitMQ.
///
/// Publishes dead letters to a topic exchange with routing key: `{domain}`.
/// Exchange name: `angzarr.dlq`
pub struct AmqpDeadLetterPublisher {
    pool: Pool,
    exchange: String,
}

impl AmqpDeadLetterPublisher {
    /// DLQ exchange name.
    const DLQ_EXCHANGE: &'static str = "angzarr.dlq";

    /// Create a new AMQP DLQ publisher.
    pub async fn new(amqp_url: &str) -> Result<Self, DlqError> {
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

#[async_trait]
impl DeadLetterPublisher for AmqpDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        #[cfg(feature = "otel")]
        let start = std::time::Instant::now();

        let domain = dead_letter.domain().unwrap_or("unknown").to_string();
        let routing_key = domain.clone();
        #[cfg(feature = "otel")]
        let reason_type = dead_letter.reason_type();

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

        #[cfg(feature = "otel")]
        {
            use crate::advice::metrics::{
                backend_attr, domain_attr, reason_type_attr, DLQ_PUBLISH_DURATION,
                DLQ_PUBLISH_TOTAL,
            };
            DLQ_PUBLISH_DURATION.record(start.elapsed().as_secs_f64(), &[backend_attr("amqp")]);
            DLQ_PUBLISH_TOTAL.add(
                1,
                &[
                    domain_attr(&domain),
                    reason_type_attr(reason_type),
                    backend_attr("amqp"),
                ],
            );
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "amqp.test.rs"]
mod tests;
