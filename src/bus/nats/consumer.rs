//! NATS JetStream consumer helpers.

use std::sync::Arc;
use std::time::Duration;

use async_nats::jetstream::{
    self,
    consumer::pull::Config as ConsumerConfig,
    stream::{Config as StreamConfig, RetentionPolicy, StorageType},
    Context,
};
use futures::StreamExt;
use prost::Message;
use tokio::sync::RwLock;

use crate::bus::error::{BusError, Result};
use crate::bus::traits::EventHandler;
use crate::proto::EventBook;

/// Idle heartbeat interval for NATS pull consumers.
///
/// When no messages are available, the server sends heartbeat messages at this interval.
/// This allows detection of:
/// - Stalled consumers (consumer not processing messages)
/// - Network connectivity issues
/// - Server-side problems
///
/// If two consecutive heartbeats are missed, the consumer recreates itself.
const IDLE_HEARTBEAT: Duration = Duration::from_secs(5);

/// Ensure the NATS JetStream stream exists for a domain.
pub(super) async fn ensure_stream_for_domain(
    jetstream: &Context,
    stream_name: &str,
    subject_pattern: &str,
) -> Result<jetstream::stream::Stream> {
    // Try to get existing stream
    match jetstream.get_stream(stream_name).await {
        Ok(stream) => Ok(stream),
        Err(_) => {
            // Create stream if it doesn't exist
            jetstream
                .create_stream(StreamConfig {
                    name: stream_name.to_string(),
                    subjects: vec![subject_pattern.to_string()],
                    retention: RetentionPolicy::Limits,
                    storage: StorageType::File,
                    ..Default::default()
                })
                .await
                .map_err(|e| BusError::Subscribe(format!("Failed to create stream: {}", e)))?;

            jetstream
                .get_stream(stream_name)
                .await
                .map_err(|e| BusError::Subscribe(format!("Failed to get stream: {}", e)))
        }
    }
}

/// Process messages from a NATS consumer stream.
///
/// Spawns a task that continuously reads messages, decodes EventBooks,
/// dispatches to handlers, and acks messages.
///
/// Uses explicit heartbeat configuration for consumer health monitoring.
pub(super) fn spawn_message_consumer(
    consumer: jetstream::consumer::Consumer<ConsumerConfig>,
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
) {
    tokio::spawn(async move {
        // Use stream builder to configure heartbeat explicitly
        // This enables detection of stalled consumers and network issues
        let mut messages = match consumer.stream().heartbeat(IDLE_HEARTBEAT).messages().await {
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

            // Create consume span and extract trace context
            let consume_span = tracing::info_span!("bus.consume", subject = %msg.subject);
            #[cfg(feature = "otel")]
            if let Some(headers) = msg.headers.as_ref() {
                super::otel::nats_extract_trace_context(headers, &consume_span);
            }

            // Dispatch to handlers
            crate::bus::dispatch::dispatch_to_handlers(&handlers, &book).await;

            // Acknowledge message
            if let Err(e) = msg.ack().await {
                tracing::error!("Failed to ack message: {}", e);
            }
        }
    });
}
