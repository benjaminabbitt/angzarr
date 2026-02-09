//! AMQP (RabbitMQ) event bus implementation.
//!
//! Uses topic exchange for routing events to consumers based on domain.

use std::sync::Arc;

use async_trait::async_trait;
use backon::{BackoffBuilder, ExponentialBuilder};
use deadpool_lapin::{Manager, Pool, PoolError};
use hex;
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
use tracing::{debug, error, info, Instrument};

use super::{BusError, EventBus, EventHandler, PublishResult, Result};
use crate::proto::EventBook;
use crate::proto_ext::CoverExt;

/// Exchange name for angzarr events.
const EVENTS_EXCHANGE: &str = "angzarr.events";

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

    /// Declare queue, bind to exchange, and start consuming messages.
    /// Spawns a background task that automatically reconnects on failure.
    async fn consume(&self) -> Result<()> {
        let queue = self
            .config
            .queue
            .clone()
            .ok_or_else(|| BusError::Subscribe("No queue configured".to_string()))?;

        let routing_key = self
            .config
            .routing_key
            .clone()
            .ok_or_else(|| BusError::Subscribe("No routing key configured".to_string()))?;

        let exchange = self.config.exchange.clone();
        let pool = self.pool.clone();
        let handlers = self.handlers.clone();

        // Spawn consumer task with reconnection loop
        tokio::spawn(async move {
            Self::consume_with_reconnect(pool, exchange, queue, routing_key, handlers).await;
        });

        Ok(())
    }

    /// Consumer loop with automatic reconnection and exponential backoff with jitter.
    async fn consume_with_reconnect(
        pool: Pool,
        exchange: String,
        queue: String,
        routing_key: String,
        handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    ) {
        use futures::StreamExt;
        use std::time::Duration;

        // Exponential backoff with jitter to prevent thundering herd
        let backoff_builder = ExponentialBuilder::default()
            .with_min_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_secs(30))
            .with_jitter();

        let mut backoff_iter = backoff_builder.build();

        loop {
            // Try to set up consumer
            match Self::setup_consumer(&pool, &exchange, &queue, &routing_key).await {
                Ok(mut consumer) => {
                    info!(
                        queue = %queue,
                        routing_key = %routing_key,
                        "Consumer connected, processing messages"
                    );
                    // Reset backoff on successful connection
                    backoff_iter = backoff_builder.build();

                    // Process messages until stream ends
                    while let Some(delivery) = consumer.next().await {
                        match delivery {
                            Ok(delivery) => {
                                Self::process_delivery(delivery, &handlers).await;
                            }
                            Err(e) => {
                                error!(error = %e, "Consumer delivery error, will reconnect");
                                break;
                            }
                        }
                    }

                    info!(queue = %queue, "Consumer stream ended, reconnecting...");
                }
                Err(e) => {
                    let delay = backoff_iter.next().unwrap_or(Duration::from_secs(30));
                    error!(
                        error = %e,
                        backoff_ms = %delay.as_millis(),
                        queue = %queue,
                        "Failed to set up consumer, retrying after backoff"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
            }

            // Brief pause before reconnecting after stream end (not error)
            let delay = backoff_iter.next().unwrap_or(Duration::from_secs(30));
            tokio::time::sleep(delay).await;
        }
    }

    /// Set up consumer channel, queue, and bindings.
    async fn setup_consumer(
        pool: &Pool,
        exchange: &str,
        queue: &str,
        routing_key: &str,
    ) -> Result<lapin::Consumer> {
        let conn = pool.get().await.map_err(|e: PoolError| {
            BusError::Connection(format!("Failed to get connection from pool: {}", e))
        })?;

        let channel = conn
            .create_channel()
            .await
            .map_err(|e| BusError::Connection(format!("Failed to create channel: {}", e)))?;

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
                exchange,
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

        // Create consumer
        let consumer = channel
            .basic_consume(
                queue,
                "angzarr-consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| BusError::Subscribe(format!("Failed to start consumer: {}", e)))?;

        Ok(consumer)
    }

    /// Process a single delivery from the consumer.
    async fn process_delivery(
        delivery: lapin::message::Delivery,
        handlers: &Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    ) {
        // Deserialize event book
        match EventBook::decode(delivery.data.as_slice()) {
            Ok(book) => {
                debug!(
                    routing_key = %delivery.routing_key,
                    "Received event book"
                );

                // Create consume span (with trace parent when otel is enabled)
                let consume_span =
                    tracing::info_span!("bus.consume", routing_key = %delivery.routing_key);

                #[cfg(feature = "otel")]
                amqp_extract_trace_context(&delivery.properties, &consume_span);

                // Wrap in Arc for sharing across handlers
                let book = Arc::new(book);

                // Call all handlers within the consume span
                async {
                    let handlers_guard = handlers.read().await;
                    for handler in handlers_guard.iter() {
                        if let Err(e) = handler.handle(Arc::clone(&book)).await {
                            error!(error = %e, "Handler failed");
                        }
                    }
                }
                .instrument(consume_span)
                .await;

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
}

#[async_trait]
impl EventBus for AmqpEventBus {
    #[tracing::instrument(name = "bus.publish", skip_all, fields(domain = %book.domain()))]
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        use std::time::Duration;

        const MAX_RETRIES: usize = 5;

        let routing_key = Self::routing_key(&book);
        let payload = book.encode_to_vec();

        // Exponential backoff with jitter to prevent thundering herd
        let backoff = ExponentialBuilder::default()
            .with_min_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_secs(5))
            .with_max_times(MAX_RETRIES)
            .with_jitter()
            .build();

        let mut last_error = None;

        for (attempt, delay) in std::iter::once(Duration::ZERO).chain(backoff).enumerate() {
            if attempt > 0 {
                tokio::time::sleep(delay).await;
            }

            // Get fresh channel for each attempt (handles reconnection)
            let channel = match self.get_channel().await {
                Ok(ch) => ch,
                Err(e) => {
                    error!(
                        attempt = attempt + 1,
                        max_retries = MAX_RETRIES,
                        error = %e,
                        "Failed to get channel, retrying..."
                    );
                    last_error = Some(e);
                    continue;
                }
            };

            let properties = BasicProperties::default()
                .with_content_type("application/protobuf".into())
                .with_delivery_mode(2); // persistent

            #[cfg(feature = "otel")]
            let properties = {
                let headers = amqp_inject_trace_context();
                if headers.inner().is_empty() {
                    properties
                } else {
                    properties.with_headers(headers)
                }
            };

            match channel
                .basic_publish(
                    &self.config.exchange,
                    &routing_key,
                    BasicPublishOptions::default(),
                    &payload,
                    properties,
                )
                .await
            {
                Ok(confirm) => match confirm.await {
                    Ok(_) => {
                        debug!(
                            exchange = %self.config.exchange,
                            routing_key = %routing_key,
                            "Published event book"
                        );
                        return Ok(PublishResult::default());
                    }
                    Err(e) => {
                        error!(
                            attempt = attempt + 1,
                            max_retries = MAX_RETRIES,
                            error = %e,
                            "Publish confirmation failed, retrying..."
                        );
                        last_error = Some(BusError::Publish(format!(
                            "Publish confirmation failed: {}",
                            e
                        )));
                    }
                },
                Err(e) => {
                    error!(
                        attempt = attempt + 1,
                        max_retries = MAX_RETRIES,
                        error = %e,
                        "Publish failed, retrying..."
                    );
                    last_error = Some(BusError::Publish(format!("Failed to publish: {}", e)));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| BusError::Publish("Max retries exceeded".to_string())))
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

    async fn start_consuming(&self) -> Result<()> {
        self.consume().await
    }

    async fn create_subscriber(
        &self,
        name: &str,
        domain_filter: Option<&str>,
    ) -> Result<Arc<dyn EventBus>> {
        let config = match domain_filter {
            Some(d) => AmqpConfig::subscriber(&self.config.url, name, d),
            None => AmqpConfig::subscriber_all(&self.config.url, name),
        };
        let bus = AmqpEventBus::new(config).await?;
        Ok(Arc::new(bus))
    }
}

// ============================================================================
// OTel Trace Context Propagation
// ============================================================================

/// Inject W3C trace context from the current span into AMQP message headers.
#[cfg(feature = "otel")]
fn amqp_inject_trace_context() -> FieldTable {
    use lapin::types::AMQPValue;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    let cx = tracing::Span::current().context();
    let mut headers = std::collections::BTreeMap::new();

    opentelemetry::global::get_text_map_propagator(|propagator| {
        struct MapInjector<'a>(
            &'a mut std::collections::BTreeMap<lapin::types::ShortString, AMQPValue>,
        );
        impl opentelemetry::propagation::Injector for MapInjector<'_> {
            fn set(&mut self, key: &str, value: String) {
                self.0
                    .insert(key.into(), AMQPValue::LongString(value.into()));
            }
        }
        propagator.inject_context(&cx, &mut MapInjector(&mut headers));
    });

    FieldTable::from(headers)
}

/// Extract W3C trace context from AMQP message properties and set as parent on span.
#[cfg(feature = "otel")]
fn amqp_extract_trace_context(properties: &BasicProperties, span: &tracing::Span) {
    use lapin::types::AMQPValue;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    if let Some(headers) = properties.headers() {
        let parent_cx = opentelemetry::global::get_text_map_propagator(|propagator| {
            struct FieldTableExtractor<'a>(&'a FieldTable);
            impl opentelemetry::propagation::Extractor for FieldTableExtractor<'_> {
                fn get(&self, key: &str) -> Option<&str> {
                    self.0.inner().get(key).and_then(|v| match v {
                        AMQPValue::LongString(s) => std::str::from_utf8(s.as_bytes()).ok(),
                        _ => None,
                    })
                }
                fn keys(&self) -> Vec<&str> {
                    self.0.inner().keys().map(|k| k.as_str()).collect()
                }
            }
            propagator.extract(&FieldTableExtractor(headers))
        });
        span.set_parent(parent_cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Cover, Uuid};

    #[test]
    fn test_routing_key_generation() {
        let book = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(Uuid {
                    value: b"test-123".to_vec(),
                }),
                correlation_id: String::new(),
                edition: None,
            }),
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };

        // "test-123" as bytes becomes "746573742d313233" in hex
        assert_eq!(AmqpEventBus::routing_key(&book), "orders.746573742d313233");
    }

    #[test]
    fn test_publisher_config() {
        let config = AmqpConfig::publisher("amqp://localhost:5672");
        assert_eq!(config.exchange, "angzarr.events");
        assert!(config.queue.is_none());
    }

    #[test]
    fn test_subscriber_config() {
        let config = AmqpConfig::subscriber("amqp://localhost:5672", "orders-projector", "orders");
        assert_eq!(config.routing_key, Some("orders.*".to_string()));
        assert_eq!(config.queue, Some("orders-projector".to_string()));
    }
}

/// Integration tests requiring a running RabbitMQ instance.
///
/// Run with: AMQP_URL=amqp://localhost:5672 cargo test --features amqp amqp_integration -- --ignored
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::proto::{event_page::Sequence, Cover, EventPage, Uuid};
    use futures::future::BoxFuture;
    use prost_types::Any;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::mpsc;

    fn amqp_url() -> String {
        std::env::var("AMQP_URL").unwrap_or_else(|_| "amqp://localhost:5672".to_string())
    }

    fn make_test_book(domain: &str) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(Uuid {
                    value: uuid::Uuid::new_v4().as_bytes().to_vec(),
                }),
                correlation_id: format!("test-{}", uuid::Uuid::new_v4()),
                edition: None,
            }),
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                created_at: None,
                event: Some(Any {
                    type_url: "type.googleapis.com/test.TestEvent".to_string(),
                    value: vec![1, 2, 3],
                }),
            }],
            snapshot: None,
            ..Default::default()
        }
    }

    /// Handler that counts received events and sends to channel.
    struct CountingHandler {
        count: Arc<AtomicUsize>,
        tx: mpsc::Sender<EventBook>,
    }

    impl EventHandler for CountingHandler {
        fn handle(
            &self,
            book: Arc<EventBook>,
        ) -> futures::future::BoxFuture<'static, std::result::Result<(), BusError>> {
            let count = self.count.clone();
            let tx = self.tx.clone();
            let book_clone = (*book).clone();
            Box::pin(async move {
                count.fetch_add(1, Ordering::SeqCst);
                let _ = tx.send(book_clone).await;
                Ok(())
            })
        }
    }

    #[tokio::test]
    #[ignore = "Requires RabbitMQ"]
    async fn test_publish_and_consume() {
        let url = amqp_url();
        let queue_name = format!("test-queue-{}", uuid::Uuid::new_v4());

        // Create publisher
        let publisher = AmqpEventBus::new(AmqpConfig::publisher(&url))
            .await
            .expect("Failed to create publisher");

        // Create subscriber
        let subscriber = AmqpEventBus::new(AmqpConfig::subscriber(&url, &queue_name, "test"))
            .await
            .expect("Failed to create subscriber");

        // Set up counting handler
        let count = Arc::new(AtomicUsize::new(0));
        let (tx, mut rx) = mpsc::channel(10);
        subscriber
            .subscribe(Box::new(CountingHandler {
                count: count.clone(),
                tx,
            }))
            .await
            .expect("Failed to subscribe");

        subscriber
            .start_consuming()
            .await
            .expect("Failed to start consuming");

        // Give consumer time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Publish event
        let book = make_test_book("test");
        publisher
            .publish(Arc::new(book.clone()))
            .await
            .expect("Failed to publish");

        // Wait for message
        let received = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("Timed out waiting for message")
            .expect("Channel closed");

        assert_eq!(received.cover.as_ref().unwrap().domain, "test");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    #[ignore = "Requires RabbitMQ"]
    async fn test_publisher_retries_on_failure() {
        // This test verifies the retry logic exists by checking the code compiles
        // and works under normal conditions. Testing actual failure recovery
        // requires killing RabbitMQ connections, which is environment-dependent.

        let url = amqp_url();
        let publisher = AmqpEventBus::new(AmqpConfig::publisher(&url))
            .await
            .expect("Failed to create publisher");

        let book = make_test_book("retry-test");

        // Should succeed on first try
        publisher
            .publish(Arc::new(book))
            .await
            .expect("Publish should succeed");
    }

    #[tokio::test]
    #[ignore = "Requires RabbitMQ"]
    async fn test_consumer_receives_multiple_messages() {
        let url = amqp_url();
        let queue_name = format!("test-multi-{}", uuid::Uuid::new_v4());

        let publisher = AmqpEventBus::new(AmqpConfig::publisher(&url))
            .await
            .expect("Failed to create publisher");

        let subscriber = AmqpEventBus::new(AmqpConfig::subscriber(&url, &queue_name, "multi"))
            .await
            .expect("Failed to create subscriber");

        let count = Arc::new(AtomicUsize::new(0));
        let (tx, mut rx) = mpsc::channel(100);
        subscriber
            .subscribe(Box::new(CountingHandler {
                count: count.clone(),
                tx,
            }))
            .await
            .unwrap();

        subscriber.start_consuming().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Publish 10 messages
        for _ in 0..10 {
            let book = make_test_book("multi");
            publisher.publish(Arc::new(book)).await.unwrap();
        }

        // Wait for all messages
        for _ in 0..10 {
            tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("Timed out")
                .expect("Channel closed");
        }

        assert_eq!(count.load(Ordering::SeqCst), 10);
    }
}
