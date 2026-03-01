//! AWS SNS-based DLQ publisher.
//!
//! Publishes dead letters to SNS topics named `angzarr-dlq-{domain}`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_sns::types::MessageAttributeValue;
use aws_sdk_sns::Client;
use base64::prelude::*;
use prost::Message;
use tokio::sync::RwLock;
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
            let sns_config = config.sns_sqs.clone();
            Box::pin(async move {
                if dlq_type != "sns-sqs" && dlq_type != "sns_sqs" {
                    return None;
                }
                let sns_config = sns_config.unwrap_or_default();
                match SnsSqsDeadLetterPublisher::from_config(&sns_config).await {
                    Ok(publisher) => Some(Ok(Arc::new(publisher) as Arc<dyn DeadLetterPublisher>)),
                    Err(e) => Some(Err(e)),
                }
            })
        },
    }
}

/// AWS SNS-based DLQ publisher.
///
/// Publishes dead letters to SNS topics named `angzarr-dlq-{domain}`.
pub struct SnsSqsDeadLetterPublisher {
    sns: Client,
    topic_prefix: String,
    topic_arns: Arc<RwLock<HashMap<String, String>>>,
}

impl SnsSqsDeadLetterPublisher {
    /// Create a new SNS/SQS DLQ publisher.
    pub async fn new(region: Option<&str>, endpoint_url: Option<&str>) -> Result<Self, DlqError> {
        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());

        if let Some(region) = region {
            config_builder = config_builder.region(aws_config::Region::new(region.to_string()));
        }

        if let Some(endpoint) = endpoint_url {
            config_builder = config_builder.endpoint_url(endpoint);
        }

        let aws_config = config_builder.load().await;
        let sns = Client::new(&aws_config);

        info!(
            region = ?region,
            endpoint = ?endpoint_url,
            "SNS/SQS DLQ publisher connected"
        );

        Ok(Self {
            sns,
            topic_prefix: "angzarr-dlq".to_string(),
            topic_arns: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a new SNS/SQS DLQ publisher from config.
    pub async fn from_config(
        config: &super::super::config::SnsSqsDlqConfig,
    ) -> Result<Self, DlqError> {
        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());

        if let Some(ref region) = config.region {
            config_builder = config_builder.region(aws_config::Region::new(region.clone()));
        }

        if let Some(ref endpoint) = config.endpoint_url {
            config_builder = config_builder.endpoint_url(endpoint);
        }

        let aws_config = config_builder.load().await;
        let sns = Client::new(&aws_config);

        info!(
            region = ?config.region,
            endpoint = ?config.endpoint_url,
            topic_prefix = %config.topic_prefix,
            "SNS/SQS DLQ publisher connected"
        );

        Ok(Self {
            sns,
            topic_prefix: config.topic_prefix.clone(),
            topic_arns: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Build DLQ topic name for a domain.
    fn topic_for_domain(&self, domain: &str) -> String {
        let sanitized = domain.replace('.', "-");
        format!("{}-{}", self.topic_prefix, sanitized)
    }

    /// Get or create an SNS topic ARN for a domain.
    async fn get_or_create_topic(&self, domain: &str) -> Result<String, DlqError> {
        let topic_name = self.topic_for_domain(domain);

        // Check cache
        {
            let arns = self.topic_arns.read().await;
            if let Some(arn) = arns.get(&topic_name) {
                return Ok(arn.clone());
            }
        }

        // Create topic (idempotent)
        let result = self
            .sns
            .create_topic()
            .name(&topic_name)
            .send()
            .await
            .map_err(|e| DlqError::PublishFailed(format!("Failed to create SNS topic: {}", e)))?;

        let arn = result
            .topic_arn()
            .ok_or_else(|| DlqError::PublishFailed("SNS create_topic returned no ARN".to_string()))?
            .to_string();

        // Cache it
        {
            let mut arns = self.topic_arns.write().await;
            arns.insert(topic_name.clone(), arn.clone());
        }

        info!(topic = %topic_name, arn = %arn, "Created/found SNS DLQ topic");
        Ok(arn)
    }
}

#[async_trait]
impl DeadLetterPublisher for SnsSqsDeadLetterPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        #[cfg(feature = "otel")]
        let start = std::time::Instant::now();

        let domain = dead_letter.domain().unwrap_or("unknown").to_string();
        let topic_arn = self.get_or_create_topic(&domain).await?;
        #[cfg(feature = "otel")]
        let reason_type = dead_letter.reason_type();

        // Serialize to proto, then base64 encode
        let proto = dead_letter.to_proto();
        let payload = proto.encode_to_vec();
        let message = BASE64_STANDARD.encode(&payload);

        // Build message attributes
        let correlation_id = dead_letter
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        let mut attrs = HashMap::new();
        attrs.insert(
            "domain".to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&domain)
                .build()
                .map_err(|e| {
                    DlqError::PublishFailed(format!("Failed to build attribute: {}", e))
                })?,
        );
        attrs.insert(
            "correlation_id".to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&correlation_id)
                .build()
                .map_err(|e| {
                    DlqError::PublishFailed(format!("Failed to build attribute: {}", e))
                })?,
        );

        self.sns
            .publish()
            .topic_arn(&topic_arn)
            .message(&message)
            .set_message_attributes(Some(attrs))
            .send()
            .await
            .map_err(|e| DlqError::PublishFailed(format!("Failed to publish to SNS: {}", e)))?;

        info!(
            topic_arn = %topic_arn,
            domain = %domain,
            reason = %dead_letter.rejection_reason,
            "Published to SNS DLQ"
        );

        #[cfg(feature = "otel")]
        {
            use crate::advice::metrics::{
                backend_attr, domain_attr, reason_type_attr, DLQ_PUBLISH_DURATION,
                DLQ_PUBLISH_TOTAL,
            };
            DLQ_PUBLISH_DURATION.record(start.elapsed().as_secs_f64(), &[backend_attr("sns_sqs")]);
            DLQ_PUBLISH_TOTAL.add(
                1,
                &[
                    domain_attr(&domain),
                    reason_type_attr(reason_type),
                    backend_attr("sns_sqs"),
                ],
            );
        }

        Ok(())
    }
}
