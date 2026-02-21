//! NATS JetStream EventBus implementation.
//!
//! Provides publish/subscribe for events using NATS JetStream consumers.
//!
//! ## Architecture
//!
//! - **Publishing**: Events published to `{prefix}.events.{domain}.{root}.{edition}`
//! - **Subscribing**: Durable pull consumers filter by domain
//! - **Consumer Groups**: Multiple subscribers with same name share workload
//!
//! # Configuration
//!
//! ```yaml
//! messaging:
//!   type: nats
//!   nats:
//!     url: "nats://localhost:4222"
//!     stream_prefix: "angzarr"
//!     consumer_name: "my-service"
//!     # JetStream-specific
//!     replicas: 3
//!     retention: "limits"  # limits, interest, workqueue
//!     max_age_hours: 168   # 7 days
//! ```

use std::sync::Arc;

use async_nats::jetstream::{
    self,
    consumer::pull::Config as ConsumerConfig,
    stream::{Config as StreamConfig, RetentionPolicy, StorageType},
    Context,
};
use async_trait::async_trait;
use futures::StreamExt;
use prost::Message;
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

use crate::bus::{BusError, EventBus, EventHandler, PublishResult, Result};
use crate::proto::{Cover, EventBook};

/// Default subject prefix for NATS streams.
const DEFAULT_PREFIX: &str = "angzarr";

/// Default edition name.
const DEFAULT_EDITION: &str = "angzarr";

/// Header name for angzarr correlation ID.
const HEADER_CORRELATION: &str = "Angzarr-Correlation";

/// Configuration for NATS EventBus.
#[derive(Debug, Clone)]
pub struct NatsBusConfig {
    /// Subject prefix (default: "angzarr")
    pub prefix: String,
    /// Consumer/subscriber name
    pub consumer_name: Option<String>,
    /// Domain filter (None = all domains)
    pub domain_filter: Option<String>,
}

impl Default for NatsBusConfig {
    fn default() -> Self {
        Self {
            prefix: DEFAULT_PREFIX.to_string(),
            consumer_name: None,
            domain_filter: None,
        }
    }
}

/// EventBus backed by NATS JetStream.
pub struct NatsEventBus {
    client: async_nats::Client,
    jetstream: Context,
    config: NatsBusConfig,
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
}

impl NatsEventBus {
    /// Create a new NATS EventBus.
    ///
    /// # Arguments
    /// * `client` - Connected NATS client
    /// * `prefix` - Optional subject prefix (defaults to "angzarr")
    pub async fn new(
        client: async_nats::Client,
        prefix: Option<&str>,
    ) -> std::result::Result<Self, async_nats::Error> {
        let jetstream = jetstream::new(client.clone());
        Ok(Self {
            client,
            jetstream,
            config: NatsBusConfig {
                prefix: prefix.unwrap_or(DEFAULT_PREFIX).to_string(),
                ..Default::default()
            },
            handlers: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Create a new EventBus with specific config.
    pub async fn with_config(
        client: async_nats::Client,
        config: NatsBusConfig,
    ) -> std::result::Result<Self, async_nats::Error> {
        let jetstream = jetstream::new(client.clone());
        Ok(Self {
            client,
            jetstream,
            config,
            handlers: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Get the stream name for a domain.
    fn stream_name(&self, domain: &str) -> String {
        format!(
            "{}_{}",
            self.config.prefix.to_uppercase(),
            domain.to_uppercase()
        )
    }

    /// Get the subject for an aggregate.
    fn subject(&self, domain: &str, root: Uuid, edition: &str) -> String {
        format!(
            "{}.events.{}.{}.{}",
            self.config.prefix,
            domain,
            root.as_hyphenated(),
            edition
        )
    }

    /// Ensure the stream exists for a domain.
    async fn ensure_stream(&self, domain: &str) -> Result<()> {
        let stream_name = self.stream_name(domain);
        let subjects = format!("{}.events.{}.>", self.config.prefix, domain);

        match self.jetstream.get_stream(&stream_name).await {
            Ok(_) => Ok(()),
            Err(_) => {
                self.jetstream
                    .create_stream(StreamConfig {
                        name: stream_name,
                        subjects: vec![subjects],
                        retention: RetentionPolicy::Limits,
                        storage: StorageType::File,
                        ..Default::default()
                    })
                    .await
                    .map_err(|e| BusError::Publish(format!("Failed to create stream: {}", e)))?;
                Ok(())
            }
        }
    }

    /// Extract root UUID from cover.
    fn extract_root(cover: &Cover) -> Result<Uuid> {
        let root_bytes = cover
            .root
            .as_ref()
            .ok_or_else(|| BusError::Publish("Missing root UUID in cover".to_string()))?
            .value
            .as_slice();

        Uuid::from_slice(root_bytes)
            .map_err(|e| BusError::Publish(format!("Invalid root UUID: {}", e)))
    }

    /// Extract edition name from cover.
    fn extract_edition(cover: &Cover) -> &str {
        cover
            .edition
            .as_ref()
            .map(|e| e.name.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_EDITION)
    }
}

#[async_trait]
impl EventBus for NatsEventBus {
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        let cover = book
            .cover
            .as_ref()
            .ok_or_else(|| BusError::Publish("Missing cover in EventBook".to_string()))?;

        let domain = &cover.domain;
        let root = Self::extract_root(cover)?;
        let edition = Self::extract_edition(cover);
        let correlation_id = &cover.correlation_id;

        self.ensure_stream(domain).await?;

        let subject = self.subject(domain, root, edition);
        let payload = book.encode_to_vec();

        // Build headers
        let mut headers = async_nats::HeaderMap::new();
        if !correlation_id.is_empty() {
            headers.insert(HEADER_CORRELATION, correlation_id.as_str());
        }

        // Publish the EventBook
        let ack_future = self
            .jetstream
            .publish_with_headers(subject.clone(), headers, payload.into())
            .await
            .map_err(|e| BusError::Publish(format!("Failed to publish: {}", e)))?;

        ack_future
            .await
            .map_err(|e| BusError::Publish(format!("Publish ack failed: {}", e)))?;

        debug!(
            domain = %domain,
            root = %root,
            subject = %subject,
            "Published EventBook to NATS"
        );

        Ok(PublishResult::default())
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
        Ok(())
    }

    async fn start_consuming(&self) -> Result<()> {
        let consumer_name = self
            .config
            .consumer_name
            .as_ref()
            .ok_or_else(|| BusError::Subscribe("Consumer name required for consuming".to_string()))?
            .clone();

        // Determine subject filter based on domain_filter
        let subject_filter = match &self.config.domain_filter {
            Some(domain) => format!("{}.events.{}.>", self.config.prefix, domain),
            None => format!("{}.events.>", self.config.prefix),
        };

        // Get or create the stream - we need to figure out which stream to use
        // If we have a domain filter, use that domain's stream
        // Otherwise, we need a catch-all stream which is harder in NATS
        let stream_name = match &self.config.domain_filter {
            Some(domain) => self.stream_name(domain),
            None => {
                // Without a domain filter, we'd need to subscribe to multiple streams
                // For now, require a domain filter for consuming
                return Err(BusError::Subscribe(
                    "Domain filter required for consuming (NATS uses per-domain streams)"
                        .to_string(),
                ));
            }
        };

        // Ensure stream exists
        let subject_pattern = format!(
            "{}.events.{}.>",
            self.config.prefix,
            self.config.domain_filter.as_ref().unwrap()
        );
        match self.jetstream.get_stream(&stream_name).await {
            Ok(_) => {}
            Err(_) => {
                self.jetstream
                    .create_stream(StreamConfig {
                        name: stream_name.clone(),
                        subjects: vec![subject_pattern],
                        retention: RetentionPolicy::Limits,
                        storage: StorageType::File,
                        ..Default::default()
                    })
                    .await
                    .map_err(|e| BusError::Subscribe(format!("Failed to create stream: {}", e)))?;
            }
        }

        let stream = self
            .jetstream
            .get_stream(&stream_name)
            .await
            .map_err(|e| BusError::Subscribe(format!("Failed to get stream: {}", e)))?;

        // Create durable consumer
        let consumer = stream
            .get_or_create_consumer(
                &consumer_name,
                ConsumerConfig {
                    name: Some(consumer_name.clone()),
                    durable_name: Some(consumer_name.clone()),
                    filter_subject: subject_filter.clone(),
                    deliver_policy: jetstream::consumer::DeliverPolicy::All,
                    ack_policy: jetstream::consumer::AckPolicy::Explicit,
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| BusError::Subscribe(format!("Failed to create consumer: {}", e)))?;

        let handlers = self.handlers.clone();

        // Spawn consumer task
        tokio::spawn(async move {
            let mut messages = match consumer.messages().await {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Failed to get message stream: {}", e);
                    return;
                }
            };

            while let Some(msg_result) = messages.next().await {
                let msg = match msg_result {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::error!("Failed to receive message: {}", e);
                        continue;
                    }
                };

                // Decode EventBook
                let book = match EventBook::decode(msg.payload.as_ref()) {
                    Ok(b) => Arc::new(b),
                    Err(e) => {
                        tracing::error!("Failed to decode EventBook: {}", e);
                        // Ack to prevent redelivery of bad messages
                        let _ = msg.ack().await;
                        continue;
                    }
                };

                // Dispatch to handlers
                crate::bus::dispatch_to_handlers(&handlers, &book).await;

                // Acknowledge message
                if let Err(e) = msg.ack().await {
                    tracing::error!("Failed to ack message: {}", e);
                }
            }
        });

        debug!(
            consumer_name = %consumer_name,
            subject_filter = %subject_filter,
            "Started NATS consumer"
        );

        Ok(())
    }

    async fn create_subscriber(
        &self,
        name: &str,
        domain_filter: Option<&str>,
    ) -> Result<Arc<dyn EventBus>> {
        let config = NatsBusConfig {
            prefix: self.config.prefix.clone(),
            consumer_name: Some(name.to_string()),
            domain_filter: domain_filter.map(String::from),
        };

        let bus = Self::with_config(self.client.clone(), config)
            .await
            .map_err(|e| BusError::Subscribe(e.to_string()))?;

        Ok(Arc::new(bus))
    }
}
