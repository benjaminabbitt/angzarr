//! Pub/Sub consumer helpers.

use std::sync::Arc;

use gcloud_pubsub::client::Client;
use gcloud_pubsub::subscription::{Subscription, SubscriptionConfig};
use prost::Message;
use tokio::sync::RwLock;
use tracing::{debug, error, info, Instrument};

use crate::bus::error::{BusError, Result};
use crate::bus::traits::{domain_matches_any, EventHandler};
use crate::proto::EventBook;

/// Result of processing a message.
#[derive(Debug)]
pub(super) enum ProcessResult {
    /// Message processed successfully - ack it.
    Success,
    /// Message didn't match domain filter - ack it.
    Filtered,
    /// Message couldn't be decoded - ack it (can't retry bad data).
    DecodeError,
    /// Handler failed - nack to retry.
    HandlerFailed,
}

/// Process message payload with domain filtering and handler dispatch.
///
/// Returns the processing result to guide ack/nack decision.
pub(super) async fn process_message_payload(
    data: &[u8],
    domain: &str,
    handlers: &Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    filter_domains: &[String],
) -> ProcessResult {
    // Check domain filter
    if !domain_matches_any(domain, filter_domains) {
        debug!(
            domain = %domain,
            filter_domains = ?filter_domains,
            "Skipping message - domain doesn't match filter"
        );
        return ProcessResult::Filtered;
    }

    // Decode EventBook
    let book = match EventBook::decode(data) {
        Ok(b) => Arc::new(b),
        Err(e) => {
            error!(error = %e, "Failed to decode EventBook");
            return ProcessResult::DecodeError;
        }
    };

    // Dispatch to handlers
    let consume_span = tracing::info_span!("bus.consume", domain = %domain);
    let success = crate::bus::dispatch::dispatch_to_handlers_with_domain(handlers, &book, domain)
        .instrument(consume_span)
        .await;

    if success {
        ProcessResult::Success
    } else {
        ProcessResult::HandlerFailed
    }
}

/// Ensure topic and subscription exist, creating them if needed.
pub(super) async fn ensure_subscription_exists(
    client: &Client,
    topic_name: &str,
    subscription_name: &str,
) -> Result<Subscription> {
    let subscription = client.subscription(subscription_name);

    if !subscription.exists(None).await.map_err(|e| {
        BusError::Subscribe(format!("Failed to check subscription existence: {}", e))
    })? {
        // Create topic if needed
        let topic = client.topic(topic_name);
        if !topic
            .exists(None)
            .await
            .map_err(|e| BusError::Subscribe(format!("Failed to check topic existence: {}", e)))?
        {
            topic.create(None, None).await.map_err(|e| {
                BusError::Subscribe(format!("Failed to create topic {}: {}", topic_name, e))
            })?;
            info!(topic = %topic_name, "Created Pub/Sub topic");
        }

        // Create subscription
        subscription
            .create(
                topic.fully_qualified_name(),
                SubscriptionConfig::default(),
                None,
            )
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

    Ok(subscription)
}
