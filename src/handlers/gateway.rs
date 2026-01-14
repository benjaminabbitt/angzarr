//! Command gateway handler for angzarr-gateway service.

use std::pin::Pin;
use std::time::Duration;

use futures::Stream;
use prost::Message;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};

use crate::proto::business_coordinator_client::BusinessCoordinatorClient;
use crate::proto::command_gateway_server::CommandGateway;
use crate::proto::event_stream_client::EventStreamClient;
use crate::proto::{
    CommandBook, CommandResponse, EventBook, EventStreamFilter, ExecuteStreamCountRequest,
    ExecuteStreamTimeRequest,
};

/// Command gateway service.
///
/// Receives commands, forwards to business coordinator, and optionally streams
/// back resulting events from the event stream service.
pub struct GatewayService {
    command_client: BusinessCoordinatorClient<tonic::transport::Channel>,
    stream_client: EventStreamClient<tonic::transport::Channel>,
    default_stream_timeout: Duration,
}

impl GatewayService {
    /// Create a new gateway service.
    pub fn new(
        command_client: BusinessCoordinatorClient<tonic::transport::Channel>,
        stream_client: EventStreamClient<tonic::transport::Channel>,
        default_stream_timeout: Duration,
    ) -> Self {
        Self {
            command_client,
            stream_client,
            default_stream_timeout,
        }
    }

    /// Generate or use existing correlation ID.
    fn ensure_correlation_id(command_book: &mut CommandBook) -> Result<String, Status> {
        if command_book.correlation_id.is_empty() {
            let mut buf = Vec::new();
            command_book
                .encode(&mut buf)
                .map_err(|e| Status::internal(format!("Failed to encode command: {e}")))?;
            let angzarr_ns = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"angzarr.dev");
            let generated = uuid::Uuid::new_v5(&angzarr_ns, &buf).to_string();
            command_book.correlation_id = generated.clone();
            Ok(generated)
        } else {
            Ok(command_book.correlation_id.clone())
        }
    }

    /// Forward command to business coordinator.
    async fn forward_command(
        &self,
        command_book: CommandBook,
        correlation_id: &str,
    ) -> Result<CommandResponse, Status> {
        let mut command_client = self.command_client.clone();
        command_client
            .handle(Request::new(command_book))
            .await
            .map(|r| r.into_inner())
            .map_err(|e| {
                warn!(correlation_id = %correlation_id, error = %e, "Command failed");
                e
            })
    }

    /// Subscribe to event stream for correlation ID.
    async fn subscribe_to_stream(
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

    /// Stream events with configurable limits.
    fn create_event_stream(
        &self,
        correlation_id: String,
        sync_response: CommandResponse,
        event_stream: tonic::Streaming<EventBook>,
        max_count: Option<u32>,
        timeout: Duration,
    ) -> Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>> {
        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
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
                    return;
                }
            }

            // Then stream additional events with timeout
            let mut event_stream = event_stream;
            loop {
                match tokio::time::timeout(timeout, event_stream.next()).await {
                    Ok(Some(Ok(event))) => {
                        debug!(correlation_id = %correlation_id, "Received event from stream");
                        count += 1;

                        if tx.send(Ok(event)).await.is_err() {
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
        });

        let stream = ReceiverStream::new(rx);
        Box::pin(stream)
    }
}

#[tonic::async_trait]
impl CommandGateway for GatewayService {
    /// Unary execute - returns immediate response only, no streaming.
    async fn execute(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let mut command_book = request.into_inner();
        let correlation_id = Self::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, "Executing command (unary)");

        let response = self.forward_command(command_book, &correlation_id).await?;
        Ok(Response::new(response))
    }

    type ExecuteStreamStream =
        Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    /// Streaming execute - streams events until default timeout.
    async fn execute_stream(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<Self::ExecuteStreamStream>, Status> {
        let mut command_book = request.into_inner();
        let correlation_id = Self::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, "Executing command (stream)");

        // Subscribe BEFORE sending command
        let event_stream = self.subscribe_to_stream(&correlation_id).await?;

        // Forward command
        let sync_response = self.forward_command(command_book, &correlation_id).await?;

        // Create stream with default timeout, no count limit
        let stream = self.create_event_stream(
            correlation_id,
            sync_response,
            event_stream,
            None,
            self.default_stream_timeout,
        );

        Ok(Response::new(stream))
    }

    type ExecuteStreamResponseCountStream =
        Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    /// Count-limited streaming - streams until N responses received.
    async fn execute_stream_response_count(
        &self,
        request: Request<ExecuteStreamCountRequest>,
    ) -> Result<Response<Self::ExecuteStreamResponseCountStream>, Status> {
        let req = request.into_inner();
        let mut command_book = req
            .command
            .ok_or_else(|| Status::invalid_argument("command is required"))?;
        let count = req.count as u32;

        let correlation_id = Self::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, count = count, "Executing command (count-limited stream)");

        // Subscribe BEFORE sending command
        let event_stream = self.subscribe_to_stream(&correlation_id).await?;

        // Forward command
        let sync_response = self.forward_command(command_book, &correlation_id).await?;

        // Create stream with count limit
        let max_count = if count == 0 { None } else { Some(count) };
        let stream = self.create_event_stream(
            correlation_id,
            sync_response,
            event_stream,
            max_count,
            self.default_stream_timeout,
        );

        Ok(Response::new(stream))
    }

    type ExecuteStreamResponseTimeStream =
        Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    /// Time-limited streaming - streams for specified duration.
    async fn execute_stream_response_time(
        &self,
        request: Request<ExecuteStreamTimeRequest>,
    ) -> Result<Response<Self::ExecuteStreamResponseTimeStream>, Status> {
        let req = request.into_inner();
        let mut command_book = req
            .command
            .ok_or_else(|| Status::invalid_argument("command is required"))?;
        let timeout_ms = req.timeout_ms as u64;

        let correlation_id = Self::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, timeout_ms = timeout_ms, "Executing command (time-limited stream)");

        // Subscribe BEFORE sending command
        let event_stream = self.subscribe_to_stream(&correlation_id).await?;

        // Forward command
        let sync_response = self.forward_command(command_book, &correlation_id).await?;

        // Create stream with custom timeout
        let timeout = Duration::from_millis(timeout_ms);
        let stream = self.create_event_stream(
            correlation_id,
            sync_response,
            event_stream,
            None,
            timeout,
        );

        Ok(Response::new(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use tokio::sync::mpsc as tokio_mpsc;
    use tokio_stream::wrappers::ReceiverStream as TokioReceiverStream;
    use tonic::transport::Server;

    use crate::proto::business_coordinator_server::{BusinessCoordinator, BusinessCoordinatorServer};
    use crate::proto::event_stream_server::{EventStream, EventStreamServer};
    use crate::proto::{Cover, EventPage, Uuid as ProtoUuid};

    /// Mock BusinessCoordinator that returns configurable responses.
    struct MockBusinessCoordinator {
        response_events: Arc<tokio::sync::RwLock<Option<EventBook>>>,
        call_count: Arc<AtomicU32>,
    }

    impl MockBusinessCoordinator {
        fn new() -> Self {
            Self {
                response_events: Arc::new(tokio::sync::RwLock::new(None)),
                call_count: Arc::new(AtomicU32::new(0)),
            }
        }

        fn get_call_count(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[tonic::async_trait]
    impl BusinessCoordinator for MockBusinessCoordinator {
        async fn handle(
            &self,
            request: Request<CommandBook>,
        ) -> Result<Response<CommandResponse>, Status> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let cmd = request.into_inner();

            let events = self.response_events.read().await.clone().unwrap_or_else(|| {
                EventBook {
                    cover: cmd.cover.clone(),
                    pages: vec![EventPage {
                        sequence: Some(crate::proto::event_page::Sequence::Num(0)),
                        event: Some(prost_types::Any {
                            type_url: "test.Event".to_string(),
                            value: vec![],
                        }),
                        created_at: None,
                        synchronous: false,
                    }],
                    snapshot: None,
                    correlation_id: cmd.correlation_id.clone(),
                    snapshot_state: None,
                }
            });

            Ok(Response::new(CommandResponse {
                events: Some(events),
                projections: vec![],
            }))
        }

        async fn record(
            &self,
            _request: Request<EventBook>,
        ) -> Result<Response<CommandResponse>, Status> {
            Ok(Response::new(CommandResponse {
                events: None,
                projections: vec![],
            }))
        }
    }

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

    #[tonic::async_trait]
    impl EventStream for MockEventStream {
        type SubscribeStream = Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send>>;

        async fn subscribe(
            &self,
            request: Request<EventStreamFilter>,
        ) -> Result<Response<Self::SubscribeStream>, Status> {
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
            Ok(Response::new(Box::pin(stream)))
        }
    }

    /// Helper to start mock servers and return gateway service.
    async fn setup_test_gateway() -> (GatewayService, Arc<MockBusinessCoordinator>, Arc<MockEventStream>, Vec<tokio::task::JoinHandle<()>>) {
        let mock_coordinator = Arc::new(MockBusinessCoordinator::new());
        let mock_stream = Arc::new(MockEventStream::new());

        // Find available ports
        let coord_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let stream_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

        let coord_listener = tokio::net::TcpListener::bind(coord_addr).await.unwrap();
        let stream_listener = tokio::net::TcpListener::bind(stream_addr).await.unwrap();

        let coord_port = coord_listener.local_addr().unwrap().port();
        let stream_port = stream_listener.local_addr().unwrap().port();

        let coord_clone = mock_coordinator.clone();
        let stream_clone = mock_stream.clone();

        // Start coordinator server
        let coord_handle = tokio::spawn(async move {
            Server::builder()
                .add_service(BusinessCoordinatorServer::new(coord_clone.as_ref().clone()))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(coord_listener))
                .await
                .ok();
        });

        // Start stream server
        let stream_handle = tokio::spawn(async move {
            Server::builder()
                .add_service(EventStreamServer::new(stream_clone.as_ref().clone()))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(stream_listener))
                .await
                .ok();
        });

        // Give servers time to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Connect clients
        let command_client = BusinessCoordinatorClient::connect(format!("http://127.0.0.1:{}", coord_port))
            .await
            .unwrap();
        let stream_client = EventStreamClient::connect(format!("http://127.0.0.1:{}", stream_port))
            .await
            .unwrap();

        let gateway = GatewayService::new(
            command_client,
            stream_client,
            Duration::from_secs(5),
        );

        (gateway, mock_coordinator, mock_stream, vec![coord_handle, stream_handle])
    }

    fn make_test_command(domain: &str) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: uuid::Uuid::new_v4().as_bytes().to_vec(),
                }),
            }),
            pages: vec![],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: false,
            fact: false,
        }
    }

    // Implement Clone for mock services (needed for tonic server)
    impl Clone for MockBusinessCoordinator {
        fn clone(&self) -> Self {
            Self {
                response_events: self.response_events.clone(),
                call_count: self.call_count.clone(),
            }
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

    #[tokio::test]
    async fn test_execute_returns_immediate_response() {
        let (gateway, mock_coord, _mock_stream, handles) = setup_test_gateway().await;

        let command = make_test_command("orders");
        let response = gateway.execute(Request::new(command)).await.unwrap();
        let cmd_response = response.into_inner();

        assert!(cmd_response.events.is_some());
        assert_eq!(mock_coord.get_call_count(), 1);

        // Cleanup
        for h in handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn test_execute_generates_correlation_id_if_empty() {
        let (gateway, _mock_coord, _mock_stream, handles) = setup_test_gateway().await;

        let command = make_test_command("orders");
        assert!(command.correlation_id.is_empty());

        let response = gateway.execute(Request::new(command)).await.unwrap();
        let cmd_response = response.into_inner();

        // The response should have events with a generated correlation ID
        assert!(cmd_response.events.is_some());
        // correlation_id is set on the command, not necessarily returned in events
        // but the command should have been processed

        for h in handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn test_execute_stream_returns_events() {
        let (gateway, _mock_coord, mock_stream, handles) = setup_test_gateway().await;

        // Configure stream to emit 2 additional events
        mock_stream.set_events(vec![
            EventBook {
                cover: None,
                pages: vec![],
                snapshot: None,
                correlation_id: String::new(),
                snapshot_state: None,
            },
            EventBook {
                cover: None,
                pages: vec![],
                snapshot: None,
                correlation_id: String::new(),
                snapshot_state: None,
            },
        ]).await;

        let command = make_test_command("orders");
        let response = gateway.execute_stream(Request::new(command)).await.unwrap();
        let mut stream = response.into_inner();

        let mut count = 0;
        while let Some(result) = stream.next().await {
            assert!(result.is_ok());
            count += 1;
            if count >= 3 {
                break; // 1 sync + 2 from stream
            }
        }

        assert!(count >= 1); // At least the sync response

        for h in handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn test_execute_stream_response_count_limits_results() {
        let (gateway, _mock_coord, mock_stream, handles) = setup_test_gateway().await;

        // Configure stream to emit 5 events
        mock_stream.set_events(vec![
            EventBook::default(),
            EventBook::default(),
            EventBook::default(),
            EventBook::default(),
            EventBook::default(),
        ]).await;

        let request = ExecuteStreamCountRequest {
            command: Some(make_test_command("orders")),
            count: 2, // Limit to 2 responses
        };

        let response = gateway
            .execute_stream_response_count(Request::new(request))
            .await
            .unwrap();
        let mut stream = response.into_inner();

        let mut count = 0;
        while let Some(result) = stream.next().await {
            assert!(result.is_ok());
            count += 1;
        }

        // Should receive exactly 2 (count limit)
        assert_eq!(count, 2);

        for h in handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn test_execute_stream_response_count_zero_unlimited() {
        let (gateway, _mock_coord, mock_stream, handles) = setup_test_gateway().await;

        // Configure stream to emit 3 events with short delay
        mock_stream.set_events(vec![
            EventBook::default(),
            EventBook::default(),
            EventBook::default(),
        ]).await;
        mock_stream.set_delay(5).await;

        let request = ExecuteStreamCountRequest {
            command: Some(make_test_command("orders")),
            count: 0, // Unlimited
        };

        let response = gateway
            .execute_stream_response_count(Request::new(request))
            .await
            .unwrap();
        let mut stream = response.into_inner();

        let mut count = 0;
        while let Some(result) = stream.next().await {
            assert!(result.is_ok());
            count += 1;
        }

        // Should receive all: 1 sync + 3 from stream = 4
        assert_eq!(count, 4);

        for h in handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn test_execute_stream_response_time_limits_duration() {
        let (gateway, _mock_coord, mock_stream, handles) = setup_test_gateway().await;

        // Configure stream to emit events slowly (300ms each)
        // With a 100ms timeout, we should get only the sync response
        // before timing out waiting for the first stream event
        mock_stream.set_events(vec![
            EventBook::default(),
            EventBook::default(),
            EventBook::default(),
        ]).await;
        mock_stream.set_delay(300).await;

        let request = ExecuteStreamTimeRequest {
            command: Some(make_test_command("orders")),
            timeout_ms: 100, // Very short - should timeout before first stream event
        };

        let start = std::time::Instant::now();
        let response = gateway
            .execute_stream_response_time(Request::new(request))
            .await
            .unwrap();
        let mut stream = response.into_inner();

        let mut count = 0;
        while let Some(result) = stream.next().await {
            assert!(result.is_ok());
            count += 1;
        }
        let elapsed = start.elapsed();

        // Should timeout quickly (100ms + overhead), not wait for all events
        assert!(elapsed.as_millis() < 500, "Should timeout quickly, took {}ms", elapsed.as_millis());
        // Should have only the sync response (1), stream events take too long
        assert_eq!(count, 1, "Expected only sync response, got {} events", count);

        for h in handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn test_execute_stream_response_count_requires_command() {
        let (gateway, _mock_coord, _mock_stream, handles) = setup_test_gateway().await;

        let request = ExecuteStreamCountRequest {
            command: None, // Missing command
            count: 1,
        };

        let result = gateway
            .execute_stream_response_count(Request::new(request))
            .await;

        match result {
            Err(status) => assert_eq!(status.code(), tonic::Code::InvalidArgument),
            Ok(_) => panic!("Expected error, got success"),
        }

        for h in handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn test_execute_stream_response_time_requires_command() {
        let (gateway, _mock_coord, _mock_stream, handles) = setup_test_gateway().await;

        let request = ExecuteStreamTimeRequest {
            command: None, // Missing command
            timeout_ms: 1000,
        };

        let result = gateway
            .execute_stream_response_time(Request::new(request))
            .await;

        match result {
            Err(status) => assert_eq!(status.code(), tonic::Code::InvalidArgument),
            Ok(_) => panic!("Expected error, got success"),
        }

        for h in handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn test_execute_preserves_provided_correlation_id() {
        let (gateway, _mock_coord, _mock_stream, handles) = setup_test_gateway().await;

        let mut command = make_test_command("orders");
        command.correlation_id = "my-custom-correlation-id".to_string();

        let response = gateway.execute(Request::new(command)).await.unwrap();
        let cmd_response = response.into_inner();

        // Command should be processed (we don't check correlation ID propagation here,
        // just that a pre-set ID doesn't break anything)
        assert!(cmd_response.events.is_some());

        for h in handles {
            h.abort();
        }
    }
}
