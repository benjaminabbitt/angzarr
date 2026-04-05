//! AWS SNS/SQS event bus implementation.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_sns::Client as SnsClient;
use aws_sdk_sqs::Client as SqsClient;
use base64::prelude::*;
use prost::Message;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::bus::error::{BusError, Result};
use crate::bus::traits::{EventBus, EventHandler, PublishResult};
use crate::proto::EventBook;
use crate::proto_ext::{CoverExt, EventPageExt};

use super::config::SnsSqsConfig;
use super::consumer::consume_sqs_queue;
use super::{CORRELATION_ID_ATTR, DOMAIN_ATTR, ROOT_ID_ATTR};

/// AWS SNS/SQS event bus implementation.
///
/// Events are published to SNS topics named `{topic_prefix}-events-{domain}`.
/// Subscribers use SQS queues with configurable IDs.
pub struct SnsSqsEventBus {
    sns: SnsClient,
    sqs: SqsClient,
    pub(crate) config: SnsSqsConfig,
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
        super::otel::sns_inject_trace_context(&mut attrs);

        // Publish to SNS
        self.sns
            .publish()
            .topic_arn(&topic_arn)
            .message(&message)
            .set_message_attributes(Some(attrs))
            .message_group_id(&root_id) // FIFO ordering by aggregate root
            .message_deduplication_id(format!(
                "{}-{}-{}",
                domain,
                root_id,
                book.pages
                    .iter()
                    .map(|p| p.sequence_num())
                    .max()
                    .unwrap_or(0)
            ))
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

        // Spawn consumer tasks for each domain's queue
        for domain in domains {
            let queue_url = {
                let urls = self.queue_urls.read().await;
                urls.get(&self.config.queue_for_domain(&domain))
                    .cloned()
                    .ok_or_else(|| {
                        BusError::Subscribe(format!("Queue URL not found for domain: {}", domain))
                    })?
            };

            tokio::spawn(consume_sqs_queue(
                queue_url,
                domain,
                self.sqs.clone(),
                self.handlers.clone(),
                self.config.domains.clone(),
                self.config.max_messages,
                self.config.wait_time_secs,
            ));
        }

        info!(subscription_id = %subscription_id, "Started SQS consumers");
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
