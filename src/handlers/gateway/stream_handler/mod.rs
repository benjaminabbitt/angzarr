//! Stream handling for gateway service.
//!
//! Manages event stream subscriptions, filtering, forwarding, and client disconnect detection.

use std::pin::Pin;
use std::time::Duration;

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::Status;
use tracing::{debug, error, info};

use crate::proto::event_stream_client::EventStreamClient;
use crate::proto::{CommandResponse, EventBook, EventStreamFilter};
use crate::proto_ext::correlated_request;

/// Stream handler for managing event subscriptions and forwarding.
#[derive(Clone)]
pub struct StreamHandler {
    stream_client: EventStreamClient<tonic::transport::Channel>,
    default_timeout: Duration,
}

impl StreamHandler {
    /// Create a new stream handler.
    pub fn new(
        stream_client: EventStreamClient<tonic::transport::Channel>,
        default_timeout: Duration,
    ) -> Self {
        Self {
            stream_client,
            default_timeout,
        }
    }

    /// Get the default stream timeout.
    pub fn default_timeout(&self) -> Duration {
        self.default_timeout
    }

    /// Subscribe to event stream for correlation ID.
    pub async fn subscribe(
        &self,
        correlation_id: &str,
    ) -> Result<tonic::Streaming<EventBook>, Status> {
        let filter = EventStreamFilter {
            correlation_id: correlation_id.to_string(),
        };

        let mut stream_client = self.stream_client.clone();
        stream_client
            .subscribe(correlated_request(filter, correlation_id))
            .await
            .map(|r| r.into_inner())
            .map_err(|e| {
                error!(error = %e, "Failed to subscribe to event stream");
                Status::unavailable(format!("Event stream unavailable: {e}"))
            })
    }

    /// Create an event stream with configurable limits.
    ///
    /// Properly detects client disconnect and closes upstream subscriptions
    /// to avoid wasting resources streaming to disconnected clients.
    pub fn create_event_stream(
        &self,
        correlation_id: String,
        sync_response: CommandResponse,
        event_stream: tonic::Streaming<EventBook>,
        max_count: Option<u32>,
        timeout: Duration,
    ) -> Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>> {
        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
            stream_events(
                tx,
                correlation_id,
                sync_response,
                event_stream,
                max_count,
                timeout,
            )
            .await;
        });

        let stream = ReceiverStream::new(rx);
        Box::pin(stream)
    }
}

/// Stream events to client with count/time limits and disconnect detection.
async fn stream_events(
    tx: mpsc::Sender<Result<EventBook, Status>>,
    correlation_id: String,
    sync_response: CommandResponse,
    event_stream: tonic::Streaming<EventBook>,
    max_count: Option<u32>,
    timeout: Duration,
) {
    let mut count = 0u32;

    // First, send the immediate response events
    if let Some(events) = sync_response.events {
        count += 1;
        if let Some(max) = max_count {
            if max > 0 && count >= max {
                let _ = tx.send(Ok(events)).await;
                return;
            }
        }
        if tx.send(Ok(events)).await.is_err() {
            debug!(correlation_id = %correlation_id, "Client disconnected during sync response");
            return;
        }
    }

    // Then stream additional events with timeout
    // Use select! to detect client disconnect while waiting for events
    let mut event_stream = event_stream;
    loop {
        tokio::select! {
            // Client disconnected - stop streaming immediately
            _ = tx.closed() => {
                info!(correlation_id = %correlation_id, "Client disconnected, closing upstream subscription");
                break;
            }
            // Event from upstream stream (with timeout)
            result = tokio::time::timeout(timeout, event_stream.next()) => {
                match result {
                    Ok(Some(Ok(event))) => {
                        debug!(correlation_id = %correlation_id, "Received event from stream");
                        count += 1;

                        if tx.send(Ok(event)).await.is_err() {
                            debug!(correlation_id = %correlation_id, "Client disconnected during send");
                            break;
                        }

                        // Check count limit
                        if let Some(max) = max_count {
                            if max > 0 && count >= max {
                                debug!(correlation_id = %correlation_id, count = count, "Count limit reached");
                                break;
                            }
                        }
                    }
                    Ok(Some(Err(e))) => {
                        error!(correlation_id = %correlation_id, error = %e, "Event stream error");
                        let _ = tx.send(Err(e)).await;
                        break;
                    }
                    Ok(None) => {
                        info!(correlation_id = %correlation_id, "Event stream ended");
                        break;
                    }
                    Err(_) => {
                        debug!(correlation_id = %correlation_id, "Event stream timeout, closing");
                        break;
                    }
                }
            }
        }
    }
    // event_stream is dropped here, signaling upstream to close subscription
    debug!(correlation_id = %correlation_id, "Stream task ending, upstream subscription will close");
}

#[cfg(test)]
mod tests;
