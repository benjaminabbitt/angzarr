//! Google Pub/Sub event bus implementation.
//!
//! Uses topics per domain for routing events to consumers.
//! Topic naming: `{topic_prefix}-events-{domain}` (dashes for Pub/Sub compatibility)
//! Subscription naming: `{topic_prefix}-{group_id}-{domain}`
//!
//! Since Pub/Sub doesn't support hierarchical topic matching natively,
//! this implementation uses subscribe-side filtering via `domain_matches`.
//!
//! # Authentication
//!
//! Uses ADC (Application Default Credentials):
//! - Set `GOOGLE_APPLICATION_CREDENTIALS` to a service account JSON path
//! - Or `GOOGLE_APPLICATION_CREDENTIALS_JSON` with the JSON content
//! - Project ID is extracted from credentials automatically

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use google_cloud_pubsub::client::{Client, ClientConfig};
use google_cloud_pubsub::publisher::Publisher;
use google_cloud_pubsub::subscription::SubscriptionConfig;
use prost::Message;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::{
    domain_matches_any, BusError, DeadLetterHandler, DlqConfig, EventBus, EventHandler,
    FailedMessage, PublishResult, Result,
};
use crate::proto::EventBook;
use crate::proto_ext::CoverExt;

/// Attribute name for domain (for filtering).
const DOMAIN_ATTR: &str = "domain";

/// Attribute name for correlation ID.
const CORRELATION_ID_ATTR: &str = "correlation_id";

/// Attribute name for aggregate root ID.
const ROOT_ID_ATTR: &str = "root_id";

/// Attribute name for retry count.
const RETRY_COUNT_ATTR: &str = "retry_count";

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
    /// Dead letter queue configuration.
    pub dlq: DlqConfig,
    /// Ack deadline in seconds (default: 60).
    pub ack_deadline_secs: u32,
    /// Max delivery attempts before DLQ (default: 5).
    pub max_delivery_attempts: i32,
}

impl PubSubConfig {
    /// Create config for publishing only.
    pub fn publisher(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            topic_prefix: "angzarr".to_string(),
            subscription_id: None,
            domains: Vec::new(),
            dlq: DlqConfig::default(),
            ack_deadline_secs: 60,
            max_delivery_attempts: 5,
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
            dlq: DlqConfig::default(),
            ack_deadline_secs: 60,
            max_delivery_attempts: 5,
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
            dlq: DlqConfig::default(),
            ack_deadline_secs: 60,
            max_delivery_attempts: 5,
        }
    }

    /// Set topic prefix.
    pub fn with_topic_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.topic_prefix = prefix.into();
        self
    }

    /// Set custom DLQ configuration.
    pub fn with_dlq(mut self, dlq: DlqConfig) -> Self {
        self.dlq = dlq;
        self
    }

    /// Set ack deadline in seconds.
    pub fn with_ack_deadline(mut self, secs: u32) -> Self {
        self.ack_deadline_secs = secs;
        self
    }

    /// Set max delivery attempts before DLQ.
    pub fn with_max_delivery_attempts(mut self, attempts: i32) -> Self {
        self.max_delivery_attempts = attempts;
        self
    }

    /// Build the topic name for a domain.
    /// Uses dashes instead of dots for Pub/Sub compatibility.
    pub fn topic_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        format!("{}-events-{}", self.topic_prefix, sanitized)
    }

    /// Build the DLQ topic name.
    pub fn dlq_topic(&self) -> String {
        format!("{}-dlq", self.topic_prefix)
    }

    /// Build the subscription name for a domain.
    pub fn subscription_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        match &self.subscription_id {
            Some(sub_id) => format!("{}-{}-{}", self.topic_prefix, sub_id, sanitized),
            None => format!("{}-{}", self.topic_prefix, sanitized),
        }
    }

    /// Build the full topic path.
    pub fn topic_path(&self, domain: &str) -> String {
        format!(
            "projects/{}/topics/{}",
            self.project_id,
            self.topic_for_domain(domain)
        )
    }

    /// Build the full subscription path.
    pub fn subscription_path(&self, domain: &str) -> String {
        format!(
            "projects/{}/subscriptions/{}",
            self.project_id,
            self.subscription_for_domain(domain)
        )
    }
}

/// Google Pub/Sub event bus implementation.
///
/// Events are published to topics named `{topic_prefix}-events-{domain}`.
/// Subscribers use subscriptions with configurable IDs.
/// Failed messages go to a DLQ topic.
pub struct PubSubEventBus {
    client: Client,
    config: PubSubConfig,
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    dlq_handlers: Arc<RwLock<Vec<Box<dyn DeadLetterHandler>>>>,
    /// Cache of publishers per topic.
    publishers: Arc<RwLock<std::collections::HashMap<String, Publisher>>>,
}

impl PubSubEventBus {
    /// Create a new Pub/Sub event bus.
    ///
    /// Uses Application Default Credentials (ADC) for authentication.
    /// Set GOOGLE_APPLICATION_CREDENTIALS or GOOGLE_APPLICATION_CREDENTIALS_JSON.
    pub async fn new(config: PubSubConfig) -> Result<Self> {
        let client_config = ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| BusError::Connection(format!("Failed to configure Pub/Sub auth: {}", e)))?;

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
            dlq_handlers: Arc::new(RwLock::new(Vec::new())),
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
        if !topic.exists(None).await.map_err(|e| {
            BusError::Publish(format!("Failed to check topic existence: {}", e))
        })? {
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
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        let domain = book.domain();
        let root_id = book.root_id_hex().unwrap_or_default();
        let correlation_id = book.correlation_id.clone();

        let publisher = self.get_publisher(domain).await?;

        // Serialize the event book
        let data = book.encode_to_vec();

        // Build message with attributes using PubsubMessage
        use google_cloud_googleapis::pubsub::v1::PubsubMessage;
        let message = PubsubMessage {
            data: data.into(),
            ordering_key: root_id.clone(), // Ordering by aggregate root
            attributes: [
                (DOMAIN_ATTR.to_string(), domain.to_string()),
                (CORRELATION_ID_ATTR.to_string(), correlation_id.clone()),
                (ROOT_ID_ATTR.to_string(), root_id),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        // Publish
        let awaiter = publisher.publish(message).await;
        awaiter.get().await.map_err(|e| {
            BusError::Publish(format!("Failed to publish to Pub/Sub: {}", e))
        })?;

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

    async fn subscribe_dlq(&self, handler: Box<dyn DeadLetterHandler>) -> Result<()> {
        let mut handlers = self.dlq_handlers.write().await;
        handlers.push(handler);
        Ok(())
    }

    async fn send_to_dlq(&self, message: FailedMessage) -> Result<()> {
        if !self.config.dlq.enabled {
            warn!(
                domain = %message.domain,
                root_id = %message.root_id,
                "DLQ disabled, dropping failed message"
            );
            return Ok(());
        }

        let dlq_topic_name = self.config.dlq_topic();

        // Get or create DLQ topic
        let topic = self.client.topic(&dlq_topic_name);
        if !topic.exists(None).await.map_err(|e| {
            BusError::DeadLetterQueue(format!("Failed to check DLQ topic existence: {}", e))
        })? {
            topic.create(None, None).await.map_err(|e| {
                BusError::DeadLetterQueue(format!("Failed to create DLQ topic {}: {}", dlq_topic_name, e))
            })?;
            info!(topic = %dlq_topic_name, "Created DLQ topic");
        }

        let publisher = topic.new_publisher(None);

        // Serialize the failed message metadata as JSON
        let metadata = serde_json::to_string(&message)
            .map_err(|e| BusError::DeadLetterQueue(format!("Failed to serialize DLQ metadata: {}", e)))?;

        use google_cloud_googleapis::pubsub::v1::PubsubMessage;
        let pubsub_message = PubsubMessage {
            data: message.payload.clone().into(),
            ordering_key: message.root_id.clone(),
            attributes: [
                (DOMAIN_ATTR.to_string(), message.domain.clone()),
                (CORRELATION_ID_ATTR.to_string(), message.correlation_id.clone()),
                (ROOT_ID_ATTR.to_string(), message.root_id.clone()),
                ("x-angzarr-dlq-metadata".to_string(), metadata),
                ("x-angzarr-handler".to_string(), message.handler_name.clone()),
                ("x-angzarr-error".to_string(), message.error.clone()),
                ("x-angzarr-attempts".to_string(), message.attempt_count.to_string()),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let awaiter = publisher.publish(pubsub_message).await;
        awaiter.get().await.map_err(|e| {
            BusError::DeadLetterQueue(format!("Failed to publish to DLQ: {}", e))
        })?;

        info!(
            domain = %message.domain,
            root_id = %message.root_id,
            handler = %message.handler_name,
            attempts = message.attempt_count,
            "Message sent to DLQ topic"
        );

        Ok(())
    }

    fn dlq_config(&self) -> DlqConfig {
        self.config.dlq.clone()
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
            // In practice, you'd list existing topics or have a known set
            warn!("No domains specified. Subscribe-side filtering will be used for all received messages.");
            // Create a subscription to the main events topic
            vec!["events".to_string()]
        } else {
            self.config.domains.clone()
        };

        let handlers = self.handlers.clone();
        let dlq_handlers = self.dlq_handlers.clone();
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
                let sub_config = SubscriptionConfig {
                    ack_deadline_seconds: config.ack_deadline_secs as i32,
                    ..Default::default()
                };

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
            let dlq_handlers = dlq_handlers.clone();
            let dlq_config = config.dlq.clone();
            let filter_domains = filter_domains.clone();

            // Spawn consumer task for this subscription
            tokio::spawn(async move {
                info!(subscription = %subscription_name, "Starting Pub/Sub consumer");

                loop {
                    match subscription.pull(10, None).await {
                        Ok(messages) => {
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

                                // Get retry count
                                let retry_count: u32 = message
                                    .message
                                    .attributes
                                    .get(RETRY_COUNT_ATTR)
                                    .and_then(|s| s.parse().ok())
                                    .unwrap_or(0);

                                match EventBook::decode(data) {
                                    Ok(book) => {
                                        let book = Arc::new(book);
                                        let handlers = handlers.read().await;

                                        let mut success = true;
                                        for handler in handlers.iter() {
                                            if let Err(e) = handler.handle(Arc::clone(&book)).await
                                            {
                                                error!(
                                                    domain = %domain,
                                                    error = %e,
                                                    "Handler failed"
                                                );
                                                success = false;

                                                // Check if we should DLQ
                                                if retry_count >= dlq_config.max_retries {
                                                    // Send to DLQ handlers
                                                    let failed = FailedMessage::new(
                                                        &book,
                                                        e.to_string(),
                                                        "pubsub_handler",
                                                        retry_count,
                                                    );
                                                    let dlq_handlers = dlq_handlers.read().await;
                                                    for dlq_handler in dlq_handlers.iter() {
                                                        let _ = dlq_handler.handle_dlq(failed.clone()).await;
                                                    }
                                                }
                                            }
                                        }

                                        if success {
                                            let _ = message.ack().await;
                                        } else if retry_count < dlq_config.max_retries {
                                            // Nack to retry
                                            let _ = message.nack().await;
                                        } else {
                                            // Max retries exceeded, ack to remove
                                            let _ = message.ack().await;
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
                            error!(error = %e, "Failed to pull messages from Pub/Sub");
                            tokio::time::sleep(Duration::from_secs(1)).await;
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

    #[test]
    fn test_topic_path() {
        let config = PubSubConfig::publisher("my-project");
        assert_eq!(
            config.topic_path("orders"),
            "projects/my-project/topics/angzarr-events-orders"
        );
    }

    #[test]
    fn test_subscription_path() {
        let config = PubSubConfig::subscriber("my-project", "my-sub", vec![]);
        assert_eq!(
            config.subscription_path("orders"),
            "projects/my-project/subscriptions/angzarr-my-sub-orders"
        );
    }
}
