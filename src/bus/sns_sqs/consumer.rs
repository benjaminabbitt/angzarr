//! SQS consumer helpers for message processing.

use std::sync::Arc;
use std::time::Duration;

use aws_sdk_sqs::Client as SqsClient;
use backon::{BackoffBuilder, ExponentialBuilder};
use base64::prelude::*;
use prost::Message;
use tokio::sync::RwLock;
use tracing::{debug, error, info, Instrument};

use crate::bus::traits::{domain_matches_any, EventHandler};
use crate::proto::EventBook;

use super::DOMAIN_ATTR;

/// Result of processing an SQS message.
#[derive(Debug)]
pub(crate) enum SqsProcessResult {
    /// Message processed successfully - delete it.
    Success,
    /// Message didn't match domain filter - delete it.
    Filtered,
    /// Message couldn't be decoded (base64 or protobuf) - delete it.
    DecodeError,
    /// Handler failed - let visibility timeout retry.
    HandlerFailed,
}

impl SqsProcessResult {
    /// Whether to delete the message from the queue.
    pub fn should_delete(&self) -> bool {
        !matches!(self, Self::HandlerFailed)
    }
}

/// Delete an SQS message from the queue.
pub(crate) async fn delete_sqs_message(sqs: &SqsClient, queue_url: &str, receipt_handle: &str) {
    let _ = sqs
        .delete_message()
        .queue_url(queue_url)
        .receipt_handle(receipt_handle)
        .send()
        .await;
}

/// Process a single SQS message.
///
/// Handles the complete decode → filter → dispatch cycle:
/// 1. Decode base64 body
/// 2. Check domain filter
/// 3. Decode EventBook protobuf
/// 4. Dispatch to handlers
#[allow(clippy::too_many_arguments)]
pub(crate) async fn process_sqs_message(
    message: &aws_sdk_sqs::types::Message,
    handlers: &Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    filter_domains: &[String],
) -> SqsProcessResult {
    // Get message body
    let body = match message.body() {
        Some(b) => b,
        None => return SqsProcessResult::DecodeError,
    };

    // Decode base64
    let data = match BASE64_STANDARD.decode(body) {
        Ok(d) => d,
        Err(e) => {
            error!(error = %e, "Failed to decode base64 message");
            return SqsProcessResult::DecodeError;
        }
    };

    // Get domain from message attributes
    let msg_domain = message
        .message_attributes()
        .and_then(|attrs| attrs.get(DOMAIN_ATTR))
        .and_then(|v| v.string_value())
        .unwrap_or("unknown");

    // Check domain filter
    if !domain_matches_any(msg_domain, filter_domains) {
        debug!(
            domain = %msg_domain,
            filter_domains = ?filter_domains,
            "Skipping message - domain doesn't match filter"
        );
        return SqsProcessResult::Filtered;
    }

    // Decode EventBook
    let book = match EventBook::decode(data.as_slice()) {
        Ok(b) => Arc::new(b),
        Err(e) => {
            error!(error = %e, "Failed to decode EventBook");
            return SqsProcessResult::DecodeError;
        }
    };

    // Dispatch to handlers
    let consume_span = tracing::info_span!("bus.consume", domain = %msg_domain);

    #[cfg(feature = "otel")]
    super::otel::sqs_extract_trace_context(message, &consume_span);

    let success = async {
        crate::bus::dispatch::dispatch_to_handlers_with_domain(handlers, &book, msg_domain).await
    }
    .instrument(consume_span)
    .await;

    if success {
        SqsProcessResult::Success
    } else {
        SqsProcessResult::HandlerFailed
    }
}

/// Run the SQS consumer loop for a single queue.
///
/// Receives messages with long polling, processes them, and handles
/// ack/nack via message deletion or visibility timeout.
pub(crate) async fn consume_sqs_queue(
    queue_url: String,
    domain: String,
    sqs: SqsClient,
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    filter_domains: Vec<String>,
    max_messages: i32,
    wait_time_secs: i32,
) {
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

                for message in output.messages() {
                    let result = process_sqs_message(message, &handlers, &filter_domains).await;

                    if result.should_delete() {
                        if let Some(receipt) = message.receipt_handle() {
                            delete_sqs_message(&sqs, &queue_url, receipt).await;
                        }
                    }
                    // HandlerFailed: let visibility timeout expire for retry
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
}
