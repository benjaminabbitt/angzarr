//! AWS SNS/SQS event bus implementation.
//!
//! Uses SNS topics for publishing and SQS queues for subscribing.
//! Topic naming: `{topic_prefix}-events-{domain}` (dashes for AWS compatibility)
//! Queue naming: `{topic_prefix}-{subscription_id}-{domain}`
//!
//! Since SNS/SQS doesn't support hierarchical topic matching natively,
//! this implementation uses subscribe-side filtering via `domain_matches`.

mod bus;
mod config;
mod consumer;
#[cfg(feature = "otel")]
pub(crate) mod otel;

use std::sync::Arc;

use tracing::info;

use super::config::{EventBusMode, MessagingConfig};
use super::error::Result;
use super::factory::BusBackend;
use super::traits::EventBus;

// Re-exports
pub use bus::SnsSqsEventBus;
pub use config::SnsSqsConfig;

// ============================================================================
// Constants
// ============================================================================

/// Message attribute name for domain (for filtering).
pub(crate) const DOMAIN_ATTR: &str = "domain";

/// Message attribute name for correlation ID.
pub(crate) const CORRELATION_ID_ATTR: &str = "correlation_id";

/// Message attribute name for aggregate root ID.
pub(crate) const ROOT_ID_ATTR: &str = "root_id";

// ============================================================================
// Self-Registration
// ============================================================================

inventory::submit! {
    BusBackend {
        try_create: |config, mode| Box::pin(try_create(config, mode)),
    }
}

async fn try_create(
    config: &MessagingConfig,
    mode: EventBusMode,
) -> Option<Result<Arc<dyn EventBus>>> {
    if config.messaging_type != "sns-sqs" {
        return None;
    }

    let mut sns_sqs_config = match mode {
        EventBusMode::Publisher => {
            SnsSqsConfig::publisher().with_topic_prefix(&config.sns_sqs.topic_prefix)
        }
        EventBusMode::Subscriber { queue, domain } => SnsSqsConfig::subscriber(queue, vec![domain])
            .with_topic_prefix(&config.sns_sqs.topic_prefix),
        EventBusMode::SubscriberAll { queue } => {
            let domains = config.sns_sqs.domains.clone().unwrap_or_default();
            if domains.is_empty() {
                SnsSqsConfig::subscriber_all(queue)
            } else {
                SnsSqsConfig::subscriber(queue, domains)
            }
            .with_topic_prefix(&config.sns_sqs.topic_prefix)
        }
    };

    // Apply region if specified
    if let Some(ref region) = config.sns_sqs.region {
        sns_sqs_config = sns_sqs_config.with_region(region);
    }

    match SnsSqsEventBus::new(sns_sqs_config).await {
        Ok(bus) => {
            info!(messaging_type = "sns-sqs", "Event bus initialized");
            Some(Ok(Arc::new(bus)))
        }
        Err(e) => Some(Err(e)),
    }
}

#[cfg(test)]
#[path = "mod.test.rs"]
mod tests;
