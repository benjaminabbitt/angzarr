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
use backon::{BackoffBuilder, ExponentialBuilder};
use base64::prelude::*;
use prost::Message;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn, Instrument};

use super::{domain_matches_any, BusError, EventBus, EventHandler, PublishResult, Result};
use crate::proto::EventBook;
use crate::proto_ext::CoverExt;

/// Message attribute name for domain (for filtering).
const DOMAIN_ATTR: &str = "domain";

/// Message attribute name for correlation ID.
const CORRELATION_ID_ATTR: &str = "correlation_id";

/// Message attribute name for aggregate root ID.
const ROOT_ID_ATTR: &str = "root_id";

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

    /// Set visibility timeout in seconds.
    pub fn with_visibility_timeout(mut self, secs: i32) -> Self {
        self.visibility_timeout_secs = secs;
        self
    }

    /// Build the SNS topic name for a domain.
    /// Uses dashes instead of dots for AWS compatibility.
    pub fn topic_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        // Use .fifo suffix for FIFO topic support (message_group_id ordering)
        format!("{}-events-{}.fifo", self.topic_prefix, sanitized)
    }

    /// Build the SQS queue name for a domain.
    pub fn queue_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        // Use .fifo suffix for FIFO queue support (matches FIFO topics)
        match &self.subscription_id {
            Some(sub_id) => format!("{}-{}-{}.fifo", self.topic_prefix, sub_id, sanitized),
            None => format!("{}-{}.fifo", self.topic_prefix, sanitized),
        }
    }
}

/// AWS SNS/SQS event bus implementation.
///
/// Events are published to SNS topics named `{topic_prefix}-events-{domain}`.
/// Subscribers use SQS queues with configurable IDs.
pub struct SnsSqsEventBus {
    sns: SnsClient,
    sqs: SqsClient,
    config: SnsSqsConfig,
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
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
            aws_config_builder = aws_config_builder.region(aws_config::Region::new(region.clone()));
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

        // Create FIFO topic (idempotent - returns existing if already exists)
        // FIFO topics enable message_group_id for aggregate root ordering
        let result = self
            .sns
            .create_topic()
            .name(&topic_name)
            .attributes("FifoTopic", "true")
            .attributes("ContentBasedDeduplication", "false") // We provide explicit dedup IDs
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

        // Create FIFO queue (idempotent - returns existing if already exists)
        // FIFO queues maintain message ordering by message_group_id
        let result = self
            .sqs
            .create_queue()
            .queue_name(&queue_name)
            .attributes(
                aws_sdk_sqs::types::QueueAttributeName::VisibilityTimeout,
                self.config.visibility_timeout_secs.to_string(),
            )
            .attributes(
                aws_sdk_sqs::types::QueueAttributeName::FifoQueue,
                "true".to_string(),
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
            .map_err(|e| {
                BusError::Subscribe(format!("Failed to subscribe queue to topic: {}", e))
            })?;

        debug!(queue_arn = %queue_arn, topic_arn = %topic_arn, "Subscribed queue to topic");
        Ok(())
    }
}

#[async_trait]
impl EventBus for SnsSqsEventBus {
    #[tracing::instrument(name = "bus.publish", skip_all, fields(domain = %book.domain()))]
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        let domain = book.domain();
        let root_id = book.root_id_hex().unwrap_or_default();
        let correlation_id = book.correlation_id().to_string();

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

        #[cfg(feature = "otel")]
        sns_inject_trace_context(&mut attrs);

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
        let sqs = self.sqs.clone();
        let config = self.config.clone();
        let queue_urls = self.queue_urls.clone();
        let filter_domains = self.config.domains.clone();

        // Spawn consumer tasks for each domain's queue
        for domain in domains {
            let handlers = handlers.clone();
            let sqs = sqs.clone();
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

                // Exponential backoff with jitter for error recovery
                let backoff_builder = ExponentialBuilder::default()
                    .with_min_delay(Duration::from_millis(100))
                    .with_max_delay(Duration::from_secs(30))
                    .with_jitter();
                let mut backoff_iter = backoff_builder.build();

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
                            // Reset backoff on successful receive
                            backoff_iter = backoff_builder.build();

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

                                match EventBook::decode(data.as_slice()) {
                                    Ok(book) => {
                                        let consume_span = tracing::info_span!("bus.consume",
                                            domain = %msg_domain);

                                        #[cfg(feature = "otel")]
                                        sqs_extract_trace_context(message, &consume_span);

                                        let book = Arc::new(book);

                                        let success = async {
                                            crate::bus::dispatch_to_handlers_with_domain(
                                                &handlers, &book, msg_domain,
                                            )
                                            .await
                                        }
                                        .instrument(consume_span)
                                        .await;

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
                                        }
                                        // On failure, let visibility timeout expire for retry
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
                            let delay = backoff_iter.next().unwrap_or(Duration::from_secs(30));
                            error!(
                                error = %e,
                                backoff_ms = %delay.as_millis(),
                                "Failed to receive messages from SQS, retrying after backoff"
                            );
                            tokio::time::sleep(delay).await;
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

    async fn create_subscriber(
        &self,
        name: &str,
        domain_filter: Option<&str>,
    ) -> Result<Arc<dyn EventBus>> {
        let mut config = match domain_filter {
            Some(d) => SnsSqsConfig::subscriber(name, vec![d.to_string()]),
            None => SnsSqsConfig::subscriber_all(name),
        };
        // Inherit region and endpoint from parent config
        config.region = self.config.region.clone();
        config.endpoint_url = self.config.endpoint_url.clone();
        let bus = SnsSqsEventBus::new(config).await?;
        Ok(Arc::new(bus))
    }
}

// ============================================================================
// OTel Trace Context Propagation
// ============================================================================

/// Inject W3C trace context from the current span into SNS message attributes.
#[cfg(feature = "otel")]
fn sns_inject_trace_context(
    attrs: &mut HashMap<String, aws_sdk_sns::types::MessageAttributeValue>,
) {
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    let cx = tracing::Span::current().context();
    opentelemetry::global::get_text_map_propagator(|propagator| {
        struct SnsInjector<'a>(&'a mut HashMap<String, aws_sdk_sns::types::MessageAttributeValue>);
        impl opentelemetry::propagation::Injector for SnsInjector<'_> {
            fn set(&mut self, key: &str, value: String) {
                if let Ok(attr) = aws_sdk_sns::types::MessageAttributeValue::builder()
                    .data_type("String")
                    .string_value(value)
                    .build()
                {
                    self.0.insert(key.to_string(), attr);
                }
            }
        }
        propagator.inject_context(&cx, &mut SnsInjector(attrs));
    });
}

/// Extract W3C trace context from SQS message attributes and set as parent on span.
#[cfg(feature = "otel")]
fn sqs_extract_trace_context(message: &aws_sdk_sqs::types::Message, span: &tracing::Span) {
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    if let Some(attrs) = message.message_attributes() {
        let parent_cx = opentelemetry::global::get_text_map_propagator(|propagator| {
            struct SqsExtractor<'a>(&'a HashMap<String, aws_sdk_sqs::types::MessageAttributeValue>);
            impl opentelemetry::propagation::Extractor for SqsExtractor<'_> {
                fn get(&self, key: &str) -> Option<&str> {
                    self.0.get(key).and_then(|v| v.string_value())
                }
                fn keys(&self) -> Vec<&str> {
                    self.0.keys().map(|k| k.as_str()).collect()
                }
            }
            propagator.extract(&SqsExtractor(attrs))
        });
        span.set_parent(parent_cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_for_domain() {
        let config = SnsSqsConfig::publisher();
        // FIFO topics require .fifo suffix
        assert_eq!(
            config.topic_for_domain("orders"),
            "angzarr-events-orders.fifo"
        );
        assert_eq!(
            config.topic_for_domain("game.player0"),
            "angzarr-events-game-player0.fifo"
        );
    }

    #[test]
    fn test_topic_with_custom_prefix() {
        let config = SnsSqsConfig::publisher().with_topic_prefix("myapp");
        assert_eq!(
            config.topic_for_domain("orders"),
            "myapp-events-orders.fifo"
        );
    }

    #[test]
    fn test_queue_for_domain() {
        let config = SnsSqsConfig::subscriber("saga-fulfillment", vec![]);
        // FIFO queues require .fifo suffix (to match FIFO topics)
        assert_eq!(
            config.queue_for_domain("orders"),
            "angzarr-saga-fulfillment-orders.fifo"
        );
    }

    #[test]
    fn test_publisher_config() {
        let config = SnsSqsConfig::publisher();
        assert!(config.subscription_id.is_none());
        assert!(config.domains.is_empty());
    }

    #[test]
    fn test_subscriber_config() {
        let config = SnsSqsConfig::subscriber("orders-projector", vec!["orders".to_string()]);
        assert_eq!(config.subscription_id, Some("orders-projector".to_string()));
        assert_eq!(config.domains, vec!["orders".to_string()]);
    }

    #[test]
    fn test_endpoint_config() {
        let config = SnsSqsConfig::publisher()
            .with_region("us-west-2")
            .with_endpoint("http://localhost:4566");
        assert_eq!(config.region, Some("us-west-2".to_string()));
        assert_eq!(
            config.endpoint_url,
            Some("http://localhost:4566".to_string())
        );
    }
}
