//! AWS SNS/SQS event bus implementation.
//!
//! Uses SNS topics for publishing and SQS queues for subscribing.
//! Topic naming: `{topic_prefix}-events-{domain}` (dashes for AWS compatibility)
//! Queue naming: `{topic_prefix}-{subscription_id}-{domain}`
//!
//! Since SNS/SQS doesn't support hierarchical topic matching natively,
//! this implementation uses subscribe-side filtering via `domain_matches`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_sns::Client as SnsClient;
use aws_sdk_sqs::Client as SqsClient;
use base64::prelude::*;
use prost::Message;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::{
    domain_matches_any, BusError, DeadLetterHandler, DlqConfig, EventBus, EventHandler,
    FailedMessage, PublishResult, Result,
};
use crate::proto::EventBook;
use crate::proto_ext::CoverExt;

/// Message attribute name for domain (for filtering).
const DOMAIN_ATTR: &str = "domain";

/// Message attribute name for correlation ID.
const CORRELATION_ID_ATTR: &str = "correlation_id";

/// Message attribute name for aggregate root ID.
const ROOT_ID_ATTR: &str = "root_id";

/// Message attribute name for retry count.
const RETRY_COUNT_ATTR: &str = "retry_count";

/// Configuration for AWS SNS/SQS connection.
#[derive(Clone, Debug)]
pub struct SnsSqsConfig {
    /// AWS region (e.g., "us-east-1"). Uses default provider chain if not set.
    pub region: Option<String>,
    /// Custom endpoint URL (for LocalStack or testing).
    pub endpoint_url: Option<String>,
    /// Topic prefix for events (default: "angzarr").
    pub topic_prefix: String,
    /// Subscription ID suffix (consumer group equivalent).
    pub subscription_id: Option<String>,
    /// Domains to subscribe to (for consumers).
    /// Empty means all domains (subscribe-side filtering used).
    pub domains: Vec<String>,
    /// Dead letter queue configuration.
    pub dlq: DlqConfig,
    /// Visibility timeout in seconds for SQS messages (default: 30).
    pub visibility_timeout_secs: i32,
    /// Max number of messages to receive in one poll (default: 10).
    pub max_messages: i32,
    /// Wait time seconds for long polling (default: 20).
    pub wait_time_secs: i32,
}

impl SnsSqsConfig {
    /// Create config for publishing only.
    pub fn publisher() -> Self {
        Self {
            region: None,
            endpoint_url: None,
            topic_prefix: "angzarr".to_string(),
            subscription_id: None,
            domains: Vec::new(),
            dlq: DlqConfig::default(),
            visibility_timeout_secs: 30,
            max_messages: 10,
            wait_time_secs: 20,
        }
    }

    /// Create config for subscribing to specific domains.
    pub fn subscriber(subscription_id: impl Into<String>, domains: Vec<String>) -> Self {
        Self {
            region: None,
            endpoint_url: None,
            topic_prefix: "angzarr".to_string(),
            subscription_id: Some(subscription_id.into()),
            domains,
            dlq: DlqConfig::default(),
            visibility_timeout_secs: 30,
            max_messages: 10,
            wait_time_secs: 20,
        }
    }

    /// Create config for subscribing to all domains.
    pub fn subscriber_all(subscription_id: impl Into<String>) -> Self {
        Self {
            region: None,
            endpoint_url: None,
            topic_prefix: "angzarr".to_string(),
            subscription_id: Some(subscription_id.into()),
            domains: Vec::new(),
            dlq: DlqConfig::default(),
            visibility_timeout_secs: 30,
            max_messages: 10,
            wait_time_secs: 20,
        }
    }

    /// Set AWS region.
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Set custom endpoint URL (for LocalStack or testing).
    pub fn with_endpoint(mut self, url: impl Into<String>) -> Self {
        self.endpoint_url = Some(url.into());
        self
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

    /// Set visibility timeout in seconds.
    pub fn with_visibility_timeout(mut self, secs: i32) -> Self {
        self.visibility_timeout_secs = secs;
        self
    }

    /// Build the SNS topic name for a domain.
    /// Uses dashes instead of dots for AWS compatibility.
    pub fn topic_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        format!("{}-events-{}", self.topic_prefix, sanitized)
    }

    /// Build the DLQ topic name.
    pub fn dlq_topic(&self) -> String {
        format!("{}-dlq", self.topic_prefix)
    }

    /// Build the SQS queue name for a domain.
    pub fn queue_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        match &self.subscription_id {
            Some(sub_id) => format!("{}-{}-{}", self.topic_prefix, sub_id, sanitized),
            None => format!("{}-{}", self.topic_prefix, sanitized),
        }
    }

    /// Build the DLQ queue name.
    pub fn dlq_queue(&self) -> String {
        match &self.subscription_id {
            Some(sub_id) => format!("{}-{}-dlq", self.topic_prefix, sub_id),
            None => format!("{}-dlq", self.topic_prefix),
        }
    }
}

/// AWS SNS/SQS event bus implementation.
///
/// Events are published to SNS topics named `{topic_prefix}-events-{domain}`.
/// Subscribers use SQS queues with configurable IDs.
/// Failed messages go to a DLQ queue.
pub struct SnsSqsEventBus {
    sns: SnsClient,
    sqs: SqsClient,
    config: SnsSqsConfig,
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    dlq_handlers: Arc<RwLock<Vec<Box<dyn DeadLetterHandler>>>>,
    /// Cache of SNS topic ARNs by domain.
    topic_arns: Arc<RwLock<HashMap<String, String>>>,
    /// Cache of SQS queue URLs by domain.
    queue_urls: Arc<RwLock<HashMap<String, String>>>,
}

impl SnsSqsEventBus {
    /// Create a new SNS/SQS event bus.
    pub async fn new(config: SnsSqsConfig) -> Result<Self> {
        // Load AWS config
        let mut aws_config_builder = aws_config::defaults(BehaviorVersion::latest());

        if let Some(ref region) = config.region {
            aws_config_builder =
                aws_config_builder.region(aws_config::Region::new(region.clone()));
        }

        if let Some(ref endpoint) = config.endpoint_url {
            aws_config_builder = aws_config_builder.endpoint_url(endpoint);
        }

        let aws_config = aws_config_builder.load().await;

        let sns = SnsClient::new(&aws_config);
        let sqs = SqsClient::new(&aws_config);

        info!(
            region = ?config.region,
            endpoint = ?config.endpoint_url,
            topic_prefix = %config.topic_prefix,
            "Connected to AWS SNS/SQS"
        );

        Ok(Self {
            sns,
            sqs,
            config,
            handlers: Arc::new(RwLock::new(Vec::new())),
            dlq_handlers: Arc::new(RwLock::new(Vec::new())),
            topic_arns: Arc::new(RwLock::new(HashMap::new())),
            queue_urls: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get or create an SNS topic ARN for a domain.
    async fn get_or_create_topic(&self, domain: &str) -> Result<String> {
        let topic_name = self.config.topic_for_domain(domain);

        // Check cache
        {
            let arns = self.topic_arns.read().await;
            if let Some(arn) = arns.get(&topic_name) {
                return Ok(arn.clone());
            }
        }

        // Create topic (idempotent - returns existing if already exists)
        let result = self
            .sns
            .create_topic()
            .name(&topic_name)
            .send()
            .await
            .map_err(|e| BusError::Publish(format!("Failed to create SNS topic: {}", e)))?;

        let arn = result
            .topic_arn()
            .ok_or_else(|| BusError::Publish("SNS create_topic returned no ARN".to_string()))?
            .to_string();

        // Cache it
        {
            let mut arns = self.topic_arns.write().await;
            arns.insert(topic_name.clone(), arn.clone());
        }

        info!(topic = %topic_name, arn = %arn, "Created/found SNS topic");
        Ok(arn)
    }

    /// Get or create an SQS queue URL for a domain.
    async fn get_or_create_queue(&self, domain: &str) -> Result<String> {
        let queue_name = self.config.queue_for_domain(domain);

        // Check cache
        {
            let urls = self.queue_urls.read().await;
            if let Some(url) = urls.get(&queue_name) {
                return Ok(url.clone());
            }
        }

        // Create queue (idempotent - returns existing if already exists)
        let result = self
            .sqs
            .create_queue()
            .queue_name(&queue_name)
            .attributes(
                aws_sdk_sqs::types::QueueAttributeName::VisibilityTimeout,
                self.config.visibility_timeout_secs.to_string(),
            )
            .send()
            .await
            .map_err(|e| BusError::Subscribe(format!("Failed to create SQS queue: {}", e)))?;

        let url = result
            .queue_url()
            .ok_or_else(|| BusError::Subscribe("SQS create_queue returned no URL".to_string()))?
            .to_string();

        // Cache it
        {
            let mut urls = self.queue_urls.write().await;
            urls.insert(queue_name.clone(), url.clone());
        }

        info!(queue = %queue_name, url = %url, "Created/found SQS queue");
        Ok(url)
    }

    /// Subscribe an SQS queue to an SNS topic.
    async fn subscribe_queue_to_topic(&self, queue_url: &str, topic_arn: &str) -> Result<()> {
        // Get queue ARN
        let queue_attrs = self
            .sqs
            .get_queue_attributes()
            .queue_url(queue_url)
            .attribute_names(aws_sdk_sqs::types::QueueAttributeName::QueueArn)
            .send()
            .await
            .map_err(|e| BusError::Subscribe(format!("Failed to get queue ARN: {}", e)))?;

        let queue_arn = queue_attrs
            .attributes()
            .and_then(|attrs| attrs.get(&aws_sdk_sqs::types::QueueAttributeName::QueueArn))
            .ok_or_else(|| BusError::Subscribe("Queue has no ARN attribute".to_string()))?;

        // Subscribe queue to topic
        self.sns
            .subscribe()
            .topic_arn(topic_arn)
            .protocol("sqs")
            .endpoint(queue_arn)
            .attributes("RawMessageDelivery", "true")
            .send()
            .await
            .map_err(|e| BusError::Subscribe(format!("Failed to subscribe queue to topic: {}", e)))?;

        debug!(queue_arn = %queue_arn, topic_arn = %topic_arn, "Subscribed queue to topic");
        Ok(())
    }
}

#[async_trait]
impl EventBus for SnsSqsEventBus {
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        let domain = book.domain();
        let root_id = book.root_id_hex().unwrap_or_default();
        let correlation_id = book.correlation_id.clone();

        let topic_arn = self.get_or_create_topic(domain).await?;

        // Serialize the event book
        let data = book.encode_to_vec();
        let message = BASE64_STANDARD.encode(&data);

        // Build message attributes
        use aws_sdk_sns::types::MessageAttributeValue;

        let mut attrs = HashMap::new();
        attrs.insert(
            DOMAIN_ATTR.to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(domain)
                .build()
                .map_err(|e| BusError::Publish(format!("Failed to build attribute: {}", e)))?,
        );
        attrs.insert(
            CORRELATION_ID_ATTR.to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&correlation_id)
                .build()
                .map_err(|e| BusError::Publish(format!("Failed to build attribute: {}", e)))?,
        );
        attrs.insert(
            ROOT_ID_ATTR.to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&root_id)
                .build()
                .map_err(|e| BusError::Publish(format!("Failed to build attribute: {}", e)))?,
        );

        // Publish to SNS
        self.sns
            .publish()
            .topic_arn(&topic_arn)
            .message(&message)
            .set_message_attributes(Some(attrs))
            .message_group_id(&root_id) // FIFO ordering by aggregate root
            .message_deduplication_id(&format!("{}-{}", correlation_id, root_id))
            .send()
            .await
            .map_err(|e| BusError::Publish(format!("Failed to publish to SNS: {}", e)))?;

        debug!(
            domain = %domain,
            correlation_id = %correlation_id,
            topic_arn = %topic_arn,
            "Published event to SNS"
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

        let dlq_queue_name = self.config.dlq_queue();

        // Create DLQ queue if needed (idempotent)
        let result = self
            .sqs
            .create_queue()
            .queue_name(&dlq_queue_name)
            .send()
            .await
            .map_err(|e| BusError::DeadLetterQueue(format!("Failed to create DLQ queue: {}", e)))?;

        let queue_url = result
            .queue_url()
            .ok_or_else(|| BusError::DeadLetterQueue("SQS create_queue returned no URL".to_string()))?;

        // Serialize the failed message metadata as JSON
        let metadata = serde_json::to_string(&message)
            .map_err(|e| BusError::DeadLetterQueue(format!("Failed to serialize DLQ metadata: {}", e)))?;

        // Build message attributes
        use aws_sdk_sqs::types::MessageAttributeValue;

        let mut attrs = HashMap::new();
        attrs.insert(
            DOMAIN_ATTR.to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&message.domain)
                .build()
                .map_err(|e| BusError::DeadLetterQueue(format!("Failed to build attribute: {}", e)))?,
        );
        attrs.insert(
            CORRELATION_ID_ATTR.to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&message.correlation_id)
                .build()
                .map_err(|e| BusError::DeadLetterQueue(format!("Failed to build attribute: {}", e)))?,
        );
        attrs.insert(
            ROOT_ID_ATTR.to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&message.root_id)
                .build()
                .map_err(|e| BusError::DeadLetterQueue(format!("Failed to build attribute: {}", e)))?,
        );
        attrs.insert(
            "x-angzarr-dlq-metadata".to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&metadata)
                .build()
                .map_err(|e| BusError::DeadLetterQueue(format!("Failed to build attribute: {}", e)))?,
        );
        attrs.insert(
            "x-angzarr-handler".to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&message.handler_name)
                .build()
                .map_err(|e| BusError::DeadLetterQueue(format!("Failed to build attribute: {}", e)))?,
        );
        attrs.insert(
            "x-angzarr-error".to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&message.error)
                .build()
                .map_err(|e| BusError::DeadLetterQueue(format!("Failed to build attribute: {}", e)))?,
        );
        attrs.insert(
            "x-angzarr-attempts".to_string(),
            MessageAttributeValue::builder()
                .data_type("Number")
                .string_value(message.attempt_count.to_string())
                .build()
                .map_err(|e| BusError::DeadLetterQueue(format!("Failed to build attribute: {}", e)))?,
        );

        // Send to DLQ
        let body = BASE64_STANDARD.encode(&message.payload);
        self.sqs
            .send_message()
            .queue_url(queue_url)
            .message_body(&body)
            .set_message_attributes(Some(attrs))
            .send()
            .await
            .map_err(|e| BusError::DeadLetterQueue(format!("Failed to send to DLQ: {}", e)))?;

        info!(
            domain = %message.domain,
            root_id = %message.root_id,
            handler = %message.handler_name,
            attempts = message.attempt_count,
            queue = %dlq_queue_name,
            "Message sent to DLQ queue"
        );

        Ok(())
    }

    fn dlq_config(&self) -> DlqConfig {
        self.config.dlq.clone()
    }

    async fn start_consuming(&self) -> Result<()> {
        let subscription_id = self.config.subscription_id.as_ref().ok_or_else(|| {
            BusError::Subscribe(
                "No subscription_id configured. Use SnsSqsConfig::subscriber()".to_string(),
            )
        })?;

        // Determine which domains to subscribe to
        let domains: Vec<String> = if self.config.domains.is_empty() {
            // Subscribe to all - use a default "events" topic
            warn!("No domains specified. Subscribe-side filtering will be used.");
            vec!["events".to_string()]
        } else {
            self.config.domains.clone()
        };

        // Set up queues and subscriptions for each domain
        for domain in &domains {
            let topic_arn = self.get_or_create_topic(domain).await?;
            let queue_url = self.get_or_create_queue(domain).await?;
            self.subscribe_queue_to_topic(&queue_url, &topic_arn)
                .await?;
        }

        let handlers = self.handlers.clone();
        let dlq_handlers = self.dlq_handlers.clone();
        let sqs = self.sqs.clone();
        let config = self.config.clone();
        let queue_urls = self.queue_urls.clone();
        let filter_domains = self.config.domains.clone();

        // Spawn consumer tasks for each domain's queue
        for domain in domains {
            let handlers = handlers.clone();
            let dlq_handlers = dlq_handlers.clone();
            let sqs = sqs.clone();
            let dlq_config = config.dlq.clone();
            let filter_domains = filter_domains.clone();
            let max_messages = config.max_messages;
            let wait_time_secs = config.wait_time_secs;

            let queue_url = {
                let urls = queue_urls.read().await;
                urls.get(&config.queue_for_domain(&domain))
                    .cloned()
                    .ok_or_else(|| {
                        BusError::Subscribe(format!("Queue URL not found for domain: {}", domain))
                    })?
            };

            tokio::spawn(async move {
                info!(queue_url = %queue_url, domain = %domain, "Starting SQS consumer");

                loop {
                    match sqs
                        .receive_message()
                        .queue_url(&queue_url)
                        .max_number_of_messages(max_messages)
                        .wait_time_seconds(wait_time_secs)
                        .message_attribute_names("All")
                        .send()
                        .await
                    {
                        Ok(output) => {
                            let messages = output.messages();
                            for message in messages {
                                let body = match message.body() {
                                    Some(b) => b,
                                    None => continue,
                                };

                                // Decode base64 body
                                let data = match BASE64_STANDARD.decode(body) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        error!(error = %e, "Failed to decode base64 message");
                                        // Delete invalid message
                                        if let Some(receipt) = message.receipt_handle() {
                                            let _ = sqs
                                                .delete_message()
                                                .queue_url(&queue_url)
                                                .receipt_handle(receipt)
                                                .send()
                                                .await;
                                        }
                                        continue;
                                    }
                                };

                                // Get domain from message attributes for filtering
                                let msg_domain = message
                                    .message_attributes()
                                    .and_then(|attrs| attrs.get(DOMAIN_ATTR))
                                    .and_then(|v| v.string_value())
                                    .unwrap_or("unknown");

                                // Subscribe-side hierarchical filtering
                                if !domain_matches_any(msg_domain, &filter_domains) {
                                    debug!(
                                        domain = %msg_domain,
                                        filter_domains = ?filter_domains,
                                        "Skipping message - domain doesn't match filter"
                                    );
                                    // Delete to remove from queue (we don't want it)
                                    if let Some(receipt) = message.receipt_handle() {
                                        let _ = sqs
                                            .delete_message()
                                            .queue_url(&queue_url)
                                            .receipt_handle(receipt)
                                            .send()
                                            .await;
                                    }
                                    continue;
                                }

                                // Get retry count
                                let retry_count: u32 = message
                                    .message_attributes()
                                    .and_then(|attrs| attrs.get(RETRY_COUNT_ATTR))
                                    .and_then(|v| v.string_value())
                                    .and_then(|s| s.parse().ok())
                                    .unwrap_or(0);

                                match EventBook::decode(data.as_slice()) {
                                    Ok(book) => {
                                        let book = Arc::new(book);
                                        let handlers_guard = handlers.read().await;

                                        let mut success = true;
                                        for handler in handlers_guard.iter() {
                                            if let Err(e) = handler.handle(Arc::clone(&book)).await {
                                                error!(
                                                    domain = %msg_domain,
                                                    error = %e,
                                                    "Handler failed"
                                                );
                                                success = false;

                                                // Check if we should DLQ
                                                if retry_count >= dlq_config.max_retries {
                                                    let failed = FailedMessage::new(
                                                        &book,
                                                        e.to_string(),
                                                        "sns_sqs_handler",
                                                        retry_count,
                                                    );
                                                    let dlq_handlers_guard =
                                                        dlq_handlers.read().await;
                                                    for dlq_handler in dlq_handlers_guard.iter() {
                                                        let _ =
                                                            dlq_handler.handle_dlq(failed.clone()).await;
                                                    }
                                                }
                                            }
                                        }

                                        if success {
                                            // Delete successfully processed message
                                            if let Some(receipt) = message.receipt_handle() {
                                                let _ = sqs
                                                    .delete_message()
                                                    .queue_url(&queue_url)
                                                    .receipt_handle(receipt)
                                                    .send()
                                                    .await;
                                            }
                                        } else if retry_count < dlq_config.max_retries {
                                            // Let visibility timeout expire for retry
                                            debug!(
                                                retry_count = retry_count,
                                                "Message will be retried after visibility timeout"
                                            );
                                        } else {
                                            // Max retries exceeded, delete
                                            if let Some(receipt) = message.receipt_handle() {
                                                let _ = sqs
                                                    .delete_message()
                                                    .queue_url(&queue_url)
                                                    .receipt_handle(receipt)
                                                    .send()
                                                    .await;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!(error = %e, "Failed to decode EventBook");
                                        // Delete invalid message
                                        if let Some(receipt) = message.receipt_handle() {
                                            let _ = sqs
                                                .delete_message()
                                                .queue_url(&queue_url)
                                                .receipt_handle(receipt)
                                                .send()
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to receive messages from SQS");
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            });
        }

        info!(
            subscription_id = %subscription_id,
            "Started SQS consumers"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_for_domain() {
        let config = SnsSqsConfig::publisher();
        assert_eq!(config.topic_for_domain("orders"), "angzarr-events-orders");
        assert_eq!(
            config.topic_for_domain("game.player0"),
            "angzarr-events-game-player0"
        );
    }

    #[test]
    fn test_topic_with_custom_prefix() {
        let config = SnsSqsConfig::publisher().with_topic_prefix("myapp");
        assert_eq!(config.topic_for_domain("orders"), "myapp-events-orders");
    }

    #[test]
    fn test_queue_for_domain() {
        let config = SnsSqsConfig::subscriber("saga-fulfillment", vec![]);
        assert_eq!(
            config.queue_for_domain("orders"),
            "angzarr-saga-fulfillment-orders"
        );
    }

    #[test]
    fn test_dlq_queue() {
        let config = SnsSqsConfig::subscriber("my-sub", vec![]);
        assert_eq!(config.dlq_queue(), "angzarr-my-sub-dlq");
    }

    #[test]
    fn test_publisher_config() {
        let config = SnsSqsConfig::publisher();
        assert!(config.subscription_id.is_none());
        assert!(config.domains.is_empty());
        assert!(config.dlq.enabled);
    }

    #[test]
    fn test_subscriber_config() {
        let config = SnsSqsConfig::subscriber("orders-projector", vec!["orders".to_string()]);
        assert_eq!(
            config.subscription_id,
            Some("orders-projector".to_string())
        );
        assert_eq!(config.domains, vec!["orders".to_string()]);
    }

    #[test]
    fn test_endpoint_config() {
        let config = SnsSqsConfig::publisher()
            .with_region("us-west-2")
            .with_endpoint("http://localhost:4566");
        assert_eq!(config.region, Some("us-west-2".to_string()));
        assert_eq!(config.endpoint_url, Some("http://localhost:4566".to_string()));
    }
}
