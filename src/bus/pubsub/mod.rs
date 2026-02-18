//! Google Pub/Sub event bus implementation.
//!
//! Uses topics per domain for routing events to consumers.
//! Topic naming: `{topic_prefix}-events-{domain}` (dashes for Pub/Sub compatibility)
//! Subscription naming: `{topic_prefix}-{subscription_id}-{domain}`
//!
//! # Authentication
//!
//! Uses ADC (Application Default Credentials):
//! - Set `GOOGLE_APPLICATION_CREDENTIALS` to a service account JSON path
//! - Or `GOOGLE_APPLICATION_CREDENTIALS_JSON` with the JSON content
//! - For local testing: set `PUBSUB_EMULATOR_HOST` to the emulator address

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use backon::{BackoffBuilder, ExponentialBuilder};
use google_cloud_pubsub::client::{Client, ClientConfig};
use google_cloud_pubsub::publisher::Publisher;
use google_cloud_pubsub::subscription::SubscriptionConfig;
use prost::Message;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn, Instrument};

use super::{domain_matches_any, BusError, EventBus, EventHandler, PublishResult, Result};
use crate::proto::EventBook;
use crate::proto_ext::CoverExt;

/// Attribute name for domain (for filtering).
const DOMAIN_ATTR: &str = "domain";

/// Attribute name for correlation ID.
const CORRELATION_ID_ATTR: &str = "correlation_id";

/// Attribute name for aggregate root ID.
const ROOT_ID_ATTR: &str = "root_id";

/// Configuration for Google Pub/Sub connection.
#[derive(Clone, Debug)]
pub struct PubSubConfig {
    /// GCP project ID (used for topic/subscription path generation).
    pub project_id: String,
    /// Topic prefix for events (default: "angzarr").
    pub topic_prefix: String,
    /// Subscription ID suffix (consumer group equivalent).
    pub subscription_id: Option<String>,
    /// Domains to subscribe to (for consumers).
    /// Empty means all domains (requires subscription to a wildcard or specific topics).
    pub domains: Vec<String>,
}

impl PubSubConfig {
    /// Create config for publishing only.
    pub fn publisher(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            topic_prefix: "angzarr".to_string(),
            subscription_id: None,
            domains: Vec::new(),
        }
    }

    /// Create config for subscribing to specific domains.
    pub fn subscriber(
        project_id: impl Into<String>,
        subscription_id: impl Into<String>,
        domains: Vec<String>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            topic_prefix: "angzarr".to_string(),
            subscription_id: Some(subscription_id.into()),
            domains,
        }
    }

    /// Create config for subscribing to all domains.
    pub fn subscriber_all(
        project_id: impl Into<String>,
        subscription_id: impl Into<String>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            topic_prefix: "angzarr".to_string(),
            subscription_id: Some(subscription_id.into()),
            domains: Vec::new(),
        }
    }

    /// Set topic prefix.
    pub fn with_topic_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.topic_prefix = prefix.into();
        self
    }

    /// Build the topic name for a domain.
    /// Uses dashes instead of dots for Pub/Sub compatibility.
    pub fn topic_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        format!("{}-events-{}", self.topic_prefix, sanitized)
    }

    /// Build the subscription name for a domain.
    pub fn subscription_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        match &self.subscription_id {
            Some(sub_id) => format!("{}-{}-{}", self.topic_prefix, sub_id, sanitized),
            None => format!("{}-{}", self.topic_prefix, sanitized),
        }
    }
}

/// Google Pub/Sub event bus implementation.
///
/// Events are published to topics named `{topic_prefix}-events-{domain}`.
/// Subscribers use subscriptions with configurable IDs.
pub struct PubSubEventBus {
    client: Client,
    config: PubSubConfig,
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    /// Cache of publishers per topic.
    publishers: Arc<RwLock<std::collections::HashMap<String, Publisher>>>,
}

impl PubSubEventBus {
    /// Create a new Pub/Sub event bus.
    ///
    /// Uses Application Default Credentials (ADC) for authentication.
    /// Set GOOGLE_APPLICATION_CREDENTIALS or PUBSUB_EMULATOR_HOST for testing.
    pub async fn new(config: PubSubConfig) -> Result<Self> {
        let client_config = ClientConfig::default().with_auth().await.map_err(|e| {
            BusError::Connection(format!("Failed to configure Pub/Sub auth: {}", e))
        })?;

        let client = Client::new(client_config)
            .await
            .map_err(|e| BusError::Connection(format!("Failed to create Pub/Sub client: {}", e)))?;

        info!(
            project_id = %config.project_id,
            topic_prefix = %config.topic_prefix,
            "Connected to Google Pub/Sub"
        );

        Ok(Self {
            client,
            config,
            handlers: Arc::new(RwLock::new(Vec::new())),
            publishers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Get or create a publisher for a topic.
    async fn get_publisher(&self, domain: &str) -> Result<Publisher> {
        let topic_name = self.config.topic_for_domain(domain);

        // Check cache
        {
            let publishers = self.publishers.read().await;
            if let Some(publisher) = publishers.get(&topic_name) {
                return Ok(publisher.clone());
            }
        }

        // Create topic if needed and get publisher
        let topic = self.client.topic(&topic_name);

        // Check if topic exists, create if not
        if !topic
            .exists(None)
            .await
            .map_err(|e| BusError::Publish(format!("Failed to check topic existence: {}", e)))?
        {
            topic.create(None, None).await.map_err(|e| {
                BusError::Publish(format!("Failed to create topic {}: {}", topic_name, e))
            })?;
            info!(topic = %topic_name, "Created Pub/Sub topic");
        }

        let publisher = topic.new_publisher(None);

        // Cache it
        {
            let mut publishers = self.publishers.write().await;
            publishers.insert(topic_name.clone(), publisher.clone());
        }

        Ok(publisher)
    }
}

#[async_trait]
impl EventBus for PubSubEventBus {
    #[tracing::instrument(name = "bus.publish", skip_all, fields(domain = %book.domain()))]
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        let domain = book.domain();
        let root_id = book.root_id_hex().unwrap_or_default();
        let correlation_id = book.correlation_id().to_string();

        let publisher = self.get_publisher(domain).await?;

        // Serialize the event book
        let data = book.encode_to_vec();

        // Build message with attributes using PubsubMessage
        use google_cloud_googleapis::pubsub::v1::PubsubMessage;
        let ordering_key = root_id.clone();
        let attributes: std::collections::HashMap<String, String> = [
            (DOMAIN_ATTR.to_string(), domain.to_string()),
            (CORRELATION_ID_ATTR.to_string(), correlation_id.clone()),
            (ROOT_ID_ATTR.to_string(), root_id),
        ]
        .into_iter()
        .collect();

        let message = PubsubMessage {
            data: data.into(),
            ordering_key,
            attributes,
            ..Default::default()
        };

        // Publish
        let awaiter = publisher.publish(message).await;
        awaiter
            .get()
            .await
            .map_err(|e| BusError::Publish(format!("Failed to publish to Pub/Sub: {}", e)))?;

        debug!(
            domain = %domain,
            correlation_id = %correlation_id,
            "Published event to Pub/Sub"
        );

        Ok(PublishResult::default())
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
        Ok(())
    }

    async fn start_consuming(&self) -> Result<()> {
        let subscription_id = self.config.subscription_id.as_ref().ok_or_else(|| {
            BusError::Subscribe(
                "No subscription_id configured. Use PubSubConfig::subscriber()".to_string(),
            )
        })?;

        // Determine which topics to subscribe to
        let topics: Vec<String> = if self.config.domains.is_empty() {
            // Subscribe to all - need at least one topic
            warn!("No domains specified. Subscribe-side filtering will be used.");
            vec!["events".to_string()]
        } else {
            self.config.domains.clone()
        };

        let handlers = self.handlers.clone();
        let config = self.config.clone();
        let client = self.client.clone();
        let filter_domains = self.config.domains.clone();

        // Subscribe to each domain's topic
        for domain in &topics {
            let topic_name = config.topic_for_domain(domain);
            let subscription_name = config.subscription_for_domain(domain);

            // Get or create subscription
            let subscription = client.subscription(&subscription_name);

            if !subscription.exists(None).await.map_err(|e| {
                BusError::Subscribe(format!("Failed to check subscription existence: {}", e))
            })? {
                // Create subscription
                let topic = client.topic(&topic_name);
                let sub_config = SubscriptionConfig::default();

                subscription
                    .create(topic.fully_qualified_name(), sub_config, None)
                    .await
                    .map_err(|e| {
                        BusError::Subscribe(format!(
                            "Failed to create subscription {}: {}",
                            subscription_name, e
                        ))
                    })?;

                info!(
                    subscription = %subscription_name,
                    topic = %topic_name,
                    "Created Pub/Sub subscription"
                );
            }

            let handlers = handlers.clone();
            let filter_domains = filter_domains.clone();

            // Spawn consumer task for this subscription
            tokio::spawn(async move {
                info!(subscription = %subscription_name, "Starting Pub/Sub consumer");

                // Exponential backoff with jitter for error recovery
                let backoff_builder = ExponentialBuilder::default()
                    .with_min_delay(Duration::from_millis(100))
                    .with_max_delay(Duration::from_secs(30))
                    .with_jitter();
                let mut backoff_iter = backoff_builder.build();

                loop {
                    match subscription.pull(10, None).await {
                        Ok(messages) => {
                            // Reset backoff on successful pull
                            backoff_iter = backoff_builder.build();

                            for message in messages {
                                let data = message.message.data.as_slice();

                                // Get domain from attributes for filtering
                                let domain = message
                                    .message
                                    .attributes
                                    .get(DOMAIN_ATTR)
                                    .map(|s| s.as_str())
                                    .unwrap_or("unknown");

                                // Subscribe-side hierarchical filtering
                                if !domain_matches_any(domain, &filter_domains) {
                                    debug!(
                                        domain = %domain,
                                        filter_domains = ?filter_domains,
                                        "Skipping message - domain doesn't match filter"
                                    );
                                    // Ack to remove from queue (we don't want it)
                                    let _ = message.ack().await;
                                    continue;
                                }

                                match EventBook::decode(data) {
                                    Ok(book) => {
                                        let consume_span = tracing::info_span!("bus.consume",
                                            domain = %domain);

                                        let book = Arc::new(book);
                                        let handlers_ref = &handlers;
                                        let book_ref = &book;

                                        let success = async {
                                            let handlers_guard = handlers_ref.read().await;
                                            let mut ok = true;
                                            for handler in handlers_guard.iter() {
                                                if let Err(e) =
                                                    handler.handle(Arc::clone(book_ref)).await
                                                {
                                                    error!(
                                                        domain = %domain,
                                                        error = %e,
                                                        "Handler failed"
                                                    );
                                                    ok = false;
                                                }
                                            }
                                            ok
                                        }
                                        .instrument(consume_span)
                                        .await;

                                        if success {
                                            let _ = message.ack().await;
                                        } else {
                                            // Nack to retry
                                            let _ = message.nack().await;
                                        }
                                    }
                                    Err(e) => {
                                        error!(error = %e, "Failed to decode EventBook");
                                        let _ = message.ack().await; // Can't retry bad data
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let delay = backoff_iter.next().unwrap_or(Duration::from_secs(30));
                            error!(
                                error = %e,
                                backoff_ms = %delay.as_millis(),
                                "Failed to pull messages from Pub/Sub, retrying after backoff"
                            );
                            tokio::time::sleep(delay).await;
                        }
                    }
                }
            });
        }

        info!(
            subscription_id = %subscription_id,
            domains = ?topics,
            "Started Pub/Sub consumers"
        );

        Ok(())
    }

    async fn create_subscriber(
        &self,
        name: &str,
        domain_filter: Option<&str>,
    ) -> Result<Arc<dyn EventBus>> {
        let config = match domain_filter {
            Some(d) => PubSubConfig::subscriber(&self.config.project_id, name, vec![d.to_string()]),
            None => PubSubConfig::subscriber_all(&self.config.project_id, name),
        };
        let bus = PubSubEventBus::new(config).await?;
        Ok(Arc::new(bus))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_for_domain() {
        let config = PubSubConfig::publisher("my-project");
        assert_eq!(config.topic_for_domain("orders"), "angzarr-events-orders");
        assert_eq!(
            config.topic_for_domain("game.player0"),
            "angzarr-events-game-player0"
        );
    }

    #[test]
    fn test_topic_with_custom_prefix() {
        let config = PubSubConfig::publisher("my-project").with_topic_prefix("myapp");
        assert_eq!(config.topic_for_domain("orders"), "myapp-events-orders");
    }

    #[test]
    fn test_subscription_for_domain() {
        let config = PubSubConfig::subscriber("my-project", "saga-fulfillment", vec![]);
        assert_eq!(
            config.subscription_for_domain("orders"),
            "angzarr-saga-fulfillment-orders"
        );
    }
}
