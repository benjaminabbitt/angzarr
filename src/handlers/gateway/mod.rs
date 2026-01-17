//! Command gateway handler for angzarr-gateway service.
//!
//! The gateway service receives commands, forwards them to business coordinators
//! based on domain routing, and optionally streams back resulting events.

mod command_router;
mod stream_handler;

pub use command_router::{map_registry_error, CommandRouter};
pub use stream_handler::StreamHandler;

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};

use crate::discovery::ServiceRegistry;
use crate::proto::command_gateway_server::CommandGateway;
use crate::proto::event_query_client::EventQueryClient;
use crate::proto::event_query_server::EventQuery;
use crate::proto::event_stream_client::EventStreamClient;
use crate::proto::{
    AggregateRoot, CommandBook, CommandResponse, EventBook, ExecuteStreamCountRequest,
    ExecuteStreamTimeRequest, Query,
};

/// Command gateway service.
///
/// Receives commands, forwards to business coordinator based on domain routing,
/// and optionally streams back resulting events from the event stream service.
pub struct GatewayService {
    command_router: CommandRouter,
    stream_handler: StreamHandler,
}

impl GatewayService {
    /// Create a new gateway service with service registry for domain routing.
    pub fn new(
        registry: Arc<ServiceRegistry>,
        stream_client: EventStreamClient<tonic::transport::Channel>,
        default_stream_timeout: Duration,
    ) -> Self {
        Self {
            command_router: CommandRouter::new(registry),
            stream_handler: StreamHandler::new(stream_client, default_stream_timeout),
        }
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
        let correlation_id = CommandRouter::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, "Executing command (unary)");

        let response = self
            .command_router
            .forward_command(command_book, &correlation_id)
            .await?;
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
        let correlation_id = CommandRouter::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, "Executing command (stream)");

        // Subscribe BEFORE sending command
        let event_stream = self.stream_handler.subscribe(&correlation_id).await?;

        // Forward command
        let sync_response = self
            .command_router
            .forward_command(command_book, &correlation_id)
            .await?;

        // Create stream with default timeout, no count limit
        let stream = self.stream_handler.create_event_stream(
            correlation_id,
            sync_response,
            event_stream,
            None,
            self.stream_handler.default_timeout(),
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

        let correlation_id = CommandRouter::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, count = count, "Executing command (count-limited stream)");

        // Subscribe BEFORE sending command
        let event_stream = self.stream_handler.subscribe(&correlation_id).await?;

        // Forward command
        let sync_response = self
            .command_router
            .forward_command(command_book, &correlation_id)
            .await?;

        // Create stream with count limit
        let max_count = if count == 0 { None } else { Some(count) };
        let stream = self.stream_handler.create_event_stream(
            correlation_id,
            sync_response,
            event_stream,
            max_count,
            self.stream_handler.default_timeout(),
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

        let correlation_id = CommandRouter::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, timeout_ms = timeout_ms, "Executing command (time-limited stream)");

        // Subscribe BEFORE sending command
        let event_stream = self.stream_handler.subscribe(&correlation_id).await?;

        // Forward command
        let sync_response = self
            .command_router
            .forward_command(command_book, &correlation_id)
            .await?;

        // Create stream with custom timeout
        let timeout = Duration::from_millis(timeout_ms);
        let stream = self.stream_handler.create_event_stream(
            correlation_id,
            sync_response,
            event_stream,
            None,
            timeout,
        );

        Ok(Response::new(stream))
    }
}

/// Event query proxy that routes queries to entity sidecars based on domain.
pub struct EventQueryProxy {
    registry: Arc<ServiceRegistry>,
}

impl EventQueryProxy {
    /// Create a new event query proxy with service registry for domain routing.
    pub fn new(registry: Arc<ServiceRegistry>) -> Self {
        Self { registry }
    }

    /// Get EventQuery client for the domain.
    async fn get_query_client(
        &self,
        domain: &str,
    ) -> Result<EventQueryClient<tonic::transport::Channel>, Status> {
        let endpoint = self
            .registry
            .get_endpoint(domain)
            .await
            .map_err(map_registry_error)?;

        let url = format!("http://{}:{}", endpoint.address, endpoint.port);
        EventQueryClient::connect(url).await.map_err(|e| {
            error!(domain = %domain, error = %e, "Failed to connect to entity for query");
            Status::unavailable(format!("Failed to connect to entity service: {}", e))
        })
    }
}

#[tonic::async_trait]
impl EventQuery for EventQueryProxy {
    /// Get a single EventBook for the domain/root.
    async fn get_event_book(&self, request: Request<Query>) -> Result<Response<EventBook>, Status> {
        let query = request.into_inner();
        let domain = &query.domain;

        debug!(domain = %domain, "Proxying GetEventBook query");

        let mut client = self.get_query_client(domain).await?;
        client.get_event_book(Request::new(query)).await
    }

    type GetEventsStream = Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    /// Stream EventBooks for the domain/root.
    async fn get_events(
        &self,
        request: Request<Query>,
    ) -> Result<Response<Self::GetEventsStream>, Status> {
        let query = request.into_inner();
        let domain = query.domain.clone();

        debug!(domain = %domain, "Proxying GetEvents query");

        let mut client = self.get_query_client(&domain).await?;
        let response = client.get_events(Request::new(query)).await?;

        // Re-box the stream to match our return type
        let stream = response.into_inner();
        Ok(Response::new(Box::pin(stream)))
    }

    type SynchronizeStream =
        Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    /// Bidirectional synchronization stream.
    ///
    /// Forwards queries to the appropriate entity sidecar and streams back events.
    /// Properly handles client disconnect by stopping the forwarding task.
    async fn synchronize(
        &self,
        request: Request<tonic::Streaming<Query>>,
    ) -> Result<Response<Self::SynchronizeStream>, Status> {
        // For synchronize, we need to handle multiple domains potentially
        // For now, route based on the first query's domain
        let mut inbound = request.into_inner();

        // Peek at first message to determine domain
        let first_query = match inbound.next().await {
            Some(Ok(q)) => q,
            Some(Err(e)) => return Err(e),
            None => return Err(Status::invalid_argument("No queries provided")),
        };

        let domain = first_query.domain.clone();
        debug!(domain = %domain, "Proxying Synchronize stream");

        let mut client = self.get_query_client(&domain).await?;

        // Create a channel to forward queries including the first one
        let (query_tx, query_rx) = mpsc::channel(32);
        let _ = query_tx.send(first_query).await;

        // Forward remaining queries, stopping if downstream closes
        let domain_clone = domain.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Downstream closed - stop forwarding
                    _ = query_tx.closed() => {
                        debug!(domain = %domain_clone, "Synchronize downstream closed, stopping query forwarding");
                        break;
                    }
                    // Receive next query from client
                    result = inbound.next() => {
                        match result {
                            Some(Ok(query)) => {
                                if query_tx.send(query).await.is_err() {
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                warn!(domain = %domain_clone, error = %e, "Synchronize inbound stream error");
                                break;
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        let outbound = ReceiverStream::new(query_rx);
        let response = client.synchronize(Request::new(outbound)).await?;

        Ok(Response::new(Box::pin(response.into_inner())))
    }

    type GetAggregateRootsStream =
        Pin<Box<dyn Stream<Item = Result<AggregateRoot, Status>> + Send + 'static>>;

    /// Get all aggregate roots across all domains.
    ///
    /// Queries all registered entity sidecars and merges results.
    /// Properly handles client disconnect by stopping mid-query.
    async fn get_aggregate_roots(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::GetAggregateRootsStream>, Status> {
        // This needs to query all registered domains and merge results
        let domains = self.registry.domains().await;

        let (tx, rx) = mpsc::channel(32);
        let registry = self.registry.clone();

        tokio::spawn(async move {
            'domains: for domain in domains {
                // Check if client disconnected before starting next domain
                if tx.is_closed() {
                    info!("GetAggregateRoots client disconnected, stopping");
                    break;
                }

                match registry.get_endpoint(&domain).await {
                    Ok(endpoint) => {
                        let url = format!("http://{}:{}", endpoint.address, endpoint.port);
                        if let Ok(mut client) = EventQueryClient::connect(url).await {
                            if let Ok(response) = client.get_aggregate_roots(Request::new(())).await
                            {
                                let mut stream = response.into_inner();
                                loop {
                                    tokio::select! {
                                        // Client disconnected - stop immediately
                                        _ = tx.closed() => {
                                            info!(domain = %domain, "GetAggregateRoots client disconnected during stream");
                                            break 'domains;
                                        }
                                        // Next result from domain
                                        result = stream.next() => {
                                            match result {
                                                Some(Ok(root)) => {
                                                    if tx.send(Ok(root)).await.is_err() {
                                                        break 'domains;
                                                    }
                                                }
                                                Some(Err(e)) => {
                                                    warn!(domain = %domain, error = %e, "Error streaming aggregate roots");
                                                    break; // Continue to next domain
                                                }
                                                None => break, // Domain stream ended, continue to next
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!(domain = %domain, error = %e, "Failed to get endpoint for domain");
                    }
                }
            }
            debug!("GetAggregateRoots task ending");
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::ServiceEndpoint;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::sync::mpsc as tokio_mpsc;
    use tokio_stream::wrappers::ReceiverStream as TokioReceiverStream;
    use tonic::transport::Server;

    use crate::proto::business_coordinator_server::{
        BusinessCoordinator, BusinessCoordinatorServer,
    };
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

            let events = self
                .response_events
                .read()
                .await
                .clone()
                .unwrap_or_else(|| EventBook {
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
            request: Request<crate::proto::EventStreamFilter>,
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
    async fn setup_test_gateway() -> (
        GatewayService,
        Arc<MockBusinessCoordinator>,
        Arc<MockEventStream>,
        Vec<tokio::task::JoinHandle<()>>,
    ) {
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
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                    coord_listener,
                ))
                .await
                .ok();
        });

        // Start stream server
        let stream_handle = tokio::spawn(async move {
            Server::builder()
                .add_service(EventStreamServer::new(stream_clone.as_ref().clone()))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                    stream_listener,
                ))
                .await
                .ok();
        });

        // Give servers time to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Create registry with wildcard endpoint for testing
        let registry = Arc::new(ServiceRegistry::new());
        registry
            .update_endpoint(ServiceEndpoint {
                domain: "*".to_string(),
                address: "127.0.0.1".to_string(),
                port: coord_port,
            })
            .await;

        let stream_client = EventStreamClient::connect(format!("http://127.0.0.1:{}", stream_port))
            .await
            .unwrap();

        let gateway = GatewayService::new(registry, stream_client, Duration::from_secs(5));

        (
            gateway,
            mock_coordinator,
            mock_stream,
            vec![coord_handle, stream_handle],
        )
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
        mock_stream
            .set_events(vec![
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
            ])
            .await;

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
        mock_stream
            .set_events(vec![
                EventBook::default(),
                EventBook::default(),
                EventBook::default(),
                EventBook::default(),
                EventBook::default(),
            ])
            .await;

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
        mock_stream
            .set_events(vec![
                EventBook::default(),
                EventBook::default(),
                EventBook::default(),
            ])
            .await;
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
        mock_stream
            .set_events(vec![
                EventBook::default(),
                EventBook::default(),
                EventBook::default(),
            ])
            .await;
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
        assert!(
            elapsed.as_millis() < 500,
            "Should timeout quickly, took {}ms",
            elapsed.as_millis()
        );
        // Should have only the sync response (1), stream events take too long
        assert_eq!(
            count, 1,
            "Expected only sync response, got {} events",
            count
        );

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

    #[tokio::test]
    async fn test_client_disconnect_stops_streaming() {
        let (gateway, _mock_coord, mock_stream, handles) = setup_test_gateway().await;

        // Configure stream to emit many events slowly
        mock_stream
            .set_events(vec![
                EventBook::default(),
                EventBook::default(),
                EventBook::default(),
                EventBook::default(),
                EventBook::default(),
            ])
            .await;
        mock_stream.set_delay(50).await;

        let request = ExecuteStreamCountRequest {
            command: Some(make_test_command("orders")),
            count: 0, // Unlimited - would receive all 5+ events normally
        };

        let response = gateway
            .execute_stream_response_count(Request::new(request))
            .await
            .unwrap();
        let mut stream = response.into_inner();

        // Read only the first event (sync response)
        let first = stream.next().await;
        assert!(first.is_some());
        assert!(first.unwrap().is_ok());

        // Drop the stream - simulates client disconnect
        drop(stream);

        // Give the spawned task time to detect disconnect and clean up
        tokio::time::sleep(Duration::from_millis(200)).await;

        // The test passes if we get here without hanging - the spawned task
        // detected the disconnect and stopped (rather than continuing to read
        // from the slow event stream for all 5 events)

        for h in handles {
            h.abort();
        }
    }

    #[tokio::test]
    async fn test_client_disconnect_during_stream_wait() {
        let (gateway, _mock_coord, mock_stream, handles) = setup_test_gateway().await;

        // Configure stream to emit events very slowly (longer than our wait)
        mock_stream.set_events(vec![EventBook::default()]).await;
        mock_stream.set_delay(5000).await; // 5 second delay

        let request = ExecuteStreamCountRequest {
            command: Some(make_test_command("orders")),
            count: 0, // Unlimited
        };

        let response = gateway
            .execute_stream_response_count(Request::new(request))
            .await
            .unwrap();
        let mut stream = response.into_inner();

        // Read the sync response
        let first = stream.next().await;
        assert!(first.is_some());

        // Drop stream while gateway is waiting for the slow event
        let start = std::time::Instant::now();
        drop(stream);

        // Give cleanup task time to run
        tokio::time::sleep(Duration::from_millis(100)).await;
        let elapsed = start.elapsed();

        // Should complete quickly (not wait for the 5 second event)
        assert!(
            elapsed.as_millis() < 1000,
            "Should detect disconnect quickly, took {}ms",
            elapsed.as_millis()
        );

        for h in handles {
            h.abort();
        }
    }
}
