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
use tracing::{debug, error, info, warn, Instrument};

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

    /// Subscribe to topics and start consuming messages.
    async fn consume(&self) -> Result<()> {
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
                // Subscribe to all using Kafka regex subscription.
                // The ^prefix tells rdkafka to interpret this as a regex pattern.
                // Pattern matches: {topic_prefix}.events.{any_domain}
                let pattern = format!("^{}\\.events\\..*", self.config.topic_prefix);
                info!(pattern = %pattern, "Using regex subscription for all domains");
                vec![pattern]
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

                                let consume_span = tracing::info_span!("bus.consume",
                                    topic = %message.topic(),
                                    partition = message.partition());

                                #[cfg(feature = "otel")]
                                kafka_extract_trace_context(&message, &consume_span);

                                let book = Arc::new(book);
                                async {
                                    super::dispatch_to_handlers(&handlers, &book).await;
                                }
                                .instrument(consume_span)
                                .await;

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
    #[tracing::instrument(name = "bus.publish", skip_all, fields(domain = %book.domain()))]
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

        #[cfg(feature = "otel")]
        let trace_headers = kafka_inject_trace_context();

        #[cfg(feature = "otel")]
        {
            record = record.headers(trace_headers);
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

    async fn start_consuming(&self) -> Result<()> {
        self.consume().await
    }

    async fn create_subscriber(
        &self,
        name: &str,
        domain_filter: Option<&str>,
    ) -> Result<Arc<dyn EventBus>> {
        let config = match domain_filter {
            Some(d) => KafkaEventBusConfig::subscriber(
                &self.config.bootstrap_servers,
                name,
                vec![d.to_string()],
            ),
            None => KafkaEventBusConfig::subscriber_all(&self.config.bootstrap_servers, name),
        };
        let bus = KafkaEventBus::new(config).await?;
        Ok(Arc::new(bus))
    }
}

// ============================================================================
// OTel Trace Context Propagation
// ============================================================================

/// Inject W3C trace context from the current span into Kafka message headers.
#[cfg(feature = "otel")]
fn kafka_inject_trace_context() -> rdkafka::message::OwnedHeaders {
    use rdkafka::message::OwnedHeaders;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    let cx = tracing::Span::current().context();
    let mut headers = OwnedHeaders::new();

    opentelemetry::global::get_text_map_propagator(|propagator| {
        struct HeaderInjector<'a>(&'a mut Vec<(String, String)>);
        impl opentelemetry::propagation::Injector for HeaderInjector<'_> {
            fn set(&mut self, key: &str, value: String) {
                self.0.push((key.to_string(), value));
            }
        }

        let mut pairs = Vec::new();
        propagator.inject_context(&cx, &mut HeaderInjector(&mut pairs));

        for (key, value) in pairs {
            headers = std::mem::take(&mut headers).insert(rdkafka::message::Header {
                key: &key,
                value: Some(value.as_bytes()),
            });
        }
    });

    headers
}

/// Extract W3C trace context from Kafka message headers and set as parent on span.
#[cfg(feature = "otel")]
fn kafka_extract_trace_context<M: rdkafka::message::Message>(message: &M, span: &tracing::Span) {
    use rdkafka::message::Headers;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    if let Some(headers) = message.headers() {
        let parent_cx = opentelemetry::global::get_text_map_propagator(|propagator| {
            struct KafkaExtractor<'a, H: Headers>(&'a H);
            impl<H: Headers> opentelemetry::propagation::Extractor for KafkaExtractor<'_, H> {
                fn get(&self, key: &str) -> Option<&str> {
                    for i in 0..self.0.count() {
                        if let Some(header) = self.0.get_as::<[u8]>(i) {
                            if header.key == key {
                                return header.value.and_then(|v| std::str::from_utf8(v).ok());
                            }
                        }
                    }
                    None
                }
                fn keys(&self) -> Vec<&str> {
                    let mut keys = Vec::new();
                    for i in 0..self.0.count() {
                        if let Some(header) = self.0.get_as::<[u8]>(i) {
                            keys.push(header.key);
                        }
                    }
                    keys
                }
            }
            propagator.extract(&KafkaExtractor(headers))
        });
        span.set_parent(parent_cx);
    }
}

#[cfg(test)]
mod tests;
