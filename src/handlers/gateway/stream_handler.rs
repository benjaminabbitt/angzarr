//! Stream handling for gateway service.
//!
//! Manages event stream subscriptions, filtering, forwarding, and client disconnect detection.

use std::pin::Pin;
use std::time::Duration;

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Status};
use tracing::{debug, error, info};

use crate::proto::event_stream_client::EventStreamClient;
use crate::proto::{CommandResponse, EventBook, EventStreamFilter};

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
            .subscribe(Request::new(filter))
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
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tokio::sync::mpsc as tokio_mpsc;
    use tokio_stream::wrappers::ReceiverStream as TokioReceiverStream;
    use tonic::transport::Server;

    use crate::proto::event_stream_server::{EventStream, EventStreamServer};

    /// Mock EventStream that emits configurable events.
    struct MockEventStream {
        events_to_emit: Arc<tokio::sync::RwLock<Vec<EventBook>>>,
        emit_delay_ms: Arc<tokio::sync::RwLock<u64>>,
    }

    impl MockEventStream {
        fn new() -> Self {
            Self {
                events_to_emit: Arc::new(tokio::sync::RwLock::new(vec![])),
                emit_delay_ms: Arc::new(tokio::sync::RwLock::new(10)),
            }
        }

        async fn set_events(&self, events: Vec<EventBook>) {
            *self.events_to_emit.write().await = events;
        }

        async fn set_delay(&self, delay_ms: u64) {
            *self.emit_delay_ms.write().await = delay_ms;
        }
    }

    impl Clone for MockEventStream {
        fn clone(&self) -> Self {
            Self {
                events_to_emit: self.events_to_emit.clone(),
                emit_delay_ms: self.emit_delay_ms.clone(),
            }
        }
    }

    #[tonic::async_trait]
    impl EventStream for MockEventStream {
        type SubscribeStream = Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send>>;

        async fn subscribe(
            &self,
            request: Request<EventStreamFilter>,
        ) -> Result<tonic::Response<Self::SubscribeStream>, Status> {
            let filter = request.into_inner();
            if filter.correlation_id.is_empty() {
                return Err(Status::invalid_argument("correlation_id is required"));
            }

            let events = self.events_to_emit.read().await.clone();
            let delay_ms = *self.emit_delay_ms.read().await;
            let correlation_id = filter.correlation_id;

            let (tx, rx) = tokio_mpsc::channel(32);

            tokio::spawn(async move {
                for mut event in events {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    event.correlation_id = correlation_id.clone();
                    if tx.send(Ok(event)).await.is_err() {
                        break;
                    }
                }
            });

            let stream = TokioReceiverStream::new(rx);
            Ok(tonic::Response::new(Box::pin(stream)))
        }
    }

    async fn setup_stream_handler() -> (
        StreamHandler,
        Arc<MockEventStream>,
        tokio::task::JoinHandle<()>,
    ) {
        let mock_stream = Arc::new(MockEventStream::new());

        let stream_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let stream_listener = tokio::net::TcpListener::bind(stream_addr).await.unwrap();
        let stream_port = stream_listener.local_addr().unwrap().port();

        let stream_clone = mock_stream.clone();
        let stream_handle = tokio::spawn(async move {
            Server::builder()
                .add_service(EventStreamServer::new(stream_clone.as_ref().clone()))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                    stream_listener,
                ))
                .await
                .ok();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let stream_client = EventStreamClient::connect(format!("http://127.0.0.1:{}", stream_port))
            .await
            .unwrap();

        let handler = StreamHandler::new(stream_client, Duration::from_secs(5));

        (handler, mock_stream, stream_handle)
    }

    #[tokio::test]
    async fn test_subscribe_returns_stream() {
        let (handler, _mock_stream, handle) = setup_stream_handler().await;

        let result = handler.subscribe("test-correlation-id").await;
        assert!(result.is_ok());

        handle.abort();
    }

    #[tokio::test]
    async fn test_create_event_stream_sends_sync_response() {
        let (handler, mock_stream, handle) = setup_stream_handler().await;
        mock_stream.set_events(vec![]).await;

        let event_stream = handler.subscribe("test-id").await.unwrap();
        let sync_response = CommandResponse {
            events: Some(EventBook::default()),
            projections: vec![],
        };

        let mut stream = handler.create_event_stream(
            "test-id".to_string(),
            sync_response,
            event_stream,
            None,
            Duration::from_millis(100),
        );

        let first = stream.next().await;
        assert!(first.is_some());
        assert!(first.unwrap().is_ok());

        handle.abort();
    }

    #[tokio::test]
    async fn test_create_event_stream_respects_count_limit() {
        let (handler, mock_stream, handle) = setup_stream_handler().await;
        mock_stream
            .set_events(vec![
                EventBook::default(),
                EventBook::default(),
                EventBook::default(),
            ])
            .await;
        mock_stream.set_delay(5).await;

        let event_stream = handler.subscribe("test-id").await.unwrap();
        let sync_response = CommandResponse {
            events: Some(EventBook::default()),
            projections: vec![],
        };

        let mut stream = handler.create_event_stream(
            "test-id".to_string(),
            sync_response,
            event_stream,
            Some(2), // Limit to 2
            Duration::from_secs(5),
        );

        let mut count = 0;
        while let Some(result) = stream.next().await {
            assert!(result.is_ok());
            count += 1;
        }

        assert_eq!(count, 2);

        handle.abort();
    }

    #[tokio::test]
    async fn test_client_disconnect_stops_streaming() {
        let (handler, mock_stream, handle) = setup_stream_handler().await;
        mock_stream
            .set_events(vec![
                EventBook::default(),
                EventBook::default(),
                EventBook::default(),
            ])
            .await;
        mock_stream.set_delay(100).await;

        let event_stream = handler.subscribe("test-id").await.unwrap();
        let sync_response = CommandResponse {
            events: Some(EventBook::default()),
            projections: vec![],
        };

        let mut stream = handler.create_event_stream(
            "test-id".to_string(),
            sync_response,
            event_stream,
            None,
            Duration::from_secs(5),
        );

        // Read only the first event
        let first = stream.next().await;
        assert!(first.is_some());

        // Drop the stream - simulates client disconnect
        drop(stream);

        // Give the spawned task time to detect disconnect
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Test passes if we get here without hanging

        handle.abort();
    }
}
