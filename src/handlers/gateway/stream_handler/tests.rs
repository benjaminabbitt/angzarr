use super::*;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc as tokio_mpsc;
use tokio_stream::wrappers::ReceiverStream as TokioReceiverStream;
use tonic::transport::Server;
use tonic::Request;

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
                // Set correlation_id on cover
                if let Some(ref mut cover) = event.cover {
                    cover.correlation_id = correlation_id.clone();
                }
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
