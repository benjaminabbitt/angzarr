//! AMQP (RabbitMQ) event bus implementation.
//!
//! Uses topic exchange for routing events to consumers based on domain.

use std::sync::Arc;

use hex;

use async_trait::async_trait;
use deadpool_lapin::{Manager, Pool, PoolError};
use lapin::{
    options::{
        BasicConsumeOptions, BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    types::FieldTable,
    BasicProperties, Channel, ExchangeKind,
};
use prost::Message;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::interfaces::event_bus::{BusError, EventBus, EventHandler, Result};
use crate::proto::EventBook;

/// Exchange name for evented events.
const EVENTS_EXCHANGE: &str = "evented.events";

/// Configuration for AMQP connection.
#[derive(Clone, Debug)]
pub struct AmqpConfig {
    /// AMQP connection URL (e.g., amqp://localhost:5672).
    pub url: String,
    /// Exchange name for publishing events.
    pub exchange: String,
    /// Queue name for consuming (used by subscribers).
    pub queue: Option<String>,
    /// Routing key pattern for binding (e.g., "orders.*").
    pub routing_key: Option<String>,
}

impl AmqpConfig {
    /// Create config for publishing only.
    pub fn publisher(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            exchange: EVENTS_EXCHANGE.to_string(),
            queue: None,
            routing_key: None,
        }
    }

    /// Create config for subscribing to a domain.
    pub fn subscriber(url: impl Into<String>, queue: impl Into<String>, domain: &str) -> Self {
        Self {
            url: url.into(),
            exchange: EVENTS_EXCHANGE.to_string(),
            queue: Some(queue.into()),
            routing_key: Some(format!("{}.*", domain)),
        }
    }

    /// Create config for subscribing to all events.
    pub fn subscriber_all(url: impl Into<String>, queue: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            exchange: EVENTS_EXCHANGE.to_string(),
            queue: Some(queue.into()),
            routing_key: Some("#".to_string()),
        }
    }
}

/// AMQP event bus implementation using RabbitMQ.
///
/// Events are published to a topic exchange with routing key: `{domain}.{aggregate_id}`.
/// Subscribers bind to queues with routing patterns like `orders.*` or `#` for all.
pub struct AmqpEventBus {
    pool: Pool,
    config: AmqpConfig,
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
}

impl AmqpEventBus {
    /// Create a new AMQP event bus.
    pub async fn new(config: AmqpConfig) -> Result<Self> {
        let manager = Manager::new(config.url.clone(), Default::default());
        let pool = Pool::builder(manager)
            .max_size(10)
            .build()
            .map_err(|e| BusError::Connection(format!("Failed to create pool: {}", e)))?;

        // Verify connection
        let conn = pool
            .get()
            .await
            .map_err(|e| BusError::Connection(format!("Failed to connect: {}", e)))?;

        // Create channel and declare exchange
        let channel = conn
            .create_channel()
            .await
            .map_err(|e| BusError::Connection(format!("Failed to create channel: {}", e)))?;

        channel
            .exchange_declare(
                &config.exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| BusError::Connection(format!("Failed to declare exchange: {}", e)))?;

        info!(
            exchange = %config.exchange,
            url = %config.url,
            "Connected to AMQP"
        );

        Ok(Self {
            pool,
            config,
            handlers: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Get a channel from the pool.
    async fn get_channel(&self) -> Result<Channel> {
        let conn = self.pool.get().await.map_err(|e: PoolError| {
            BusError::Connection(format!("Failed to get connection from pool: {}", e))
        })?;

        conn.create_channel()
            .await
            .map_err(|e| BusError::Connection(format!("Failed to create channel: {}", e)))
    }

    /// Build routing key from event book.
    fn routing_key(book: &EventBook) -> String {
        let domain = book
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown");

        let root_id = book
            .cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .map(|u| hex::encode(&u.value))
            .unwrap_or_else(|| "unknown".to_string());

        format!("{}.{}", domain, root_id)
    }

    /// Start consuming messages (call after subscribe).
    pub async fn start_consuming(&self) -> Result<()> {
        let queue = self
            .config
            .queue
            .as_ref()
            .ok_or_else(|| BusError::Subscribe("No queue configured".to_string()))?;

        let routing_key = self
            .config
            .routing_key
            .as_ref()
            .ok_or_else(|| BusError::Subscribe("No routing key configured".to_string()))?;

        let channel = self.get_channel().await?;

        // Declare queue
        channel
            .queue_declare(
                queue,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| BusError::Subscribe(format!("Failed to declare queue: {}", e)))?;

        // Bind queue to exchange
        channel
            .queue_bind(
                queue,
                &self.config.exchange,
                routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| BusError::Subscribe(format!("Failed to bind queue: {}", e)))?;

        info!(
            queue = %queue,
            routing_key = %routing_key,
            "Bound queue to exchange"
        );

        // Start consumer
        let mut consumer = channel
            .basic_consume(
                queue,
                "evented-consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| BusError::Subscribe(format!("Failed to start consumer: {}", e)))?;

        let handlers = self.handlers.clone();

        // Spawn consumer task
        tokio::spawn(async move {
            use futures::StreamExt;

            while let Some(delivery) = consumer.next().await {
                match delivery {
                    Ok(delivery) => {
                        // Deserialize event book
                        match EventBook::decode(delivery.data.as_slice()) {
                            Ok(book) => {
                                debug!(
                                    routing_key = %delivery.routing_key,
                                    "Received event book"
                                );

                                // Call all handlers
                                let handlers_guard = handlers.read().await;
                                for handler in handlers_guard.iter() {
                                    if let Err(e) = handler.handle(book.clone()).await {
                                        error!(error = %e, "Handler failed");
                                    }
                                }

                                // Acknowledge message
                                if let Err(e) = delivery.ack(Default::default()).await {
                                    error!(error = %e, "Failed to ack message");
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "Failed to decode event book");
                                // Reject message (don't requeue malformed messages)
                                let _ = delivery.reject(Default::default()).await;
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Consumer error");
                    }
                }
            }
        });

        Ok(())
    }
}

#[async_trait]
impl EventBus for AmqpEventBus {
    async fn publish(&self, book: &EventBook) -> Result<()> {
        let channel = self.get_channel().await?;
        let routing_key = Self::routing_key(book);

        // Serialize event book to protobuf
        let payload = book.encode_to_vec();

        channel
            .basic_publish(
                &self.config.exchange,
                &routing_key,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default()
                    .with_content_type("application/protobuf".into())
                    .with_delivery_mode(2), // persistent
            )
            .await
            .map_err(|e| BusError::Publish(format!("Failed to publish: {}", e)))?
            .await
            .map_err(|e| BusError::Publish(format!("Publish confirmation failed: {}", e)))?;

        debug!(
            exchange = %self.config.exchange,
            routing_key = %routing_key,
            "Published event book"
        );

        Ok(())
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        if self.config.queue.is_none() {
            return Err(BusError::Subscribe(
                "Cannot subscribe: no queue configured. Use AmqpConfig::subscriber()".to_string(),
            ));
        }

        let mut handlers = self.handlers.write().await;
        handlers.push(handler);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_key_generation() {
        use crate::proto::{Cover, Uuid};

        let book = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(Uuid {
                    value: b"test-123".to_vec(),
                }),
            }),
            pages: vec![],
            snapshot: None,
        };

        // "test-123" as bytes becomes "746573742d313233" in hex
        assert_eq!(AmqpEventBus::routing_key(&book), "orders.746573742d313233");
    }

    #[test]
    fn test_publisher_config() {
        let config = AmqpConfig::publisher("amqp://localhost:5672");
        assert_eq!(config.exchange, "evented.events");
        assert!(config.queue.is_none());
    }

    #[test]
    fn test_subscriber_config() {
        let config = AmqpConfig::subscriber("amqp://localhost:5672", "orders-projector", "orders");
        assert_eq!(config.routing_key, Some("orders.*".to_string()));
        assert_eq!(config.queue, Some("orders-projector".to_string()));
    }
}
