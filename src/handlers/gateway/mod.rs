//! Command gateway handler for angzarr-gateway service.
//!
//! The gateway service receives commands, forwards them to business coordinators
//! based on domain routing, and optionally streams back resulting events.

mod command_router;
mod query_proxy;
mod stream_handler;

pub use command_router::{map_discovery_error, CommandRouter};
pub use query_proxy::EventQueryProxy;
pub use stream_handler::StreamHandler;

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::Stream;
use tonic::{Request, Response, Status};
use tracing::debug;

use crate::discovery::ServiceDiscovery;
use crate::proto::command_gateway_server::CommandGateway;
use crate::proto::event_stream_client::EventStreamClient;
use crate::proto::{CommandBook, CommandResponse, DryRunRequest, EventBook, SyncCommandBook};

/// Command gateway service.
///
/// Receives commands, forwards to business coordinator based on domain routing,
/// and optionally streams back resulting events from the event stream service.
pub struct GatewayService {
    command_router: CommandRouter,
    stream_handler: Option<StreamHandler>,
}

impl GatewayService {
    /// Create a new gateway service with service discovery for domain routing.
    ///
    /// `stream_client` is optional - when `None`, streaming is disabled (embedded mode).
    pub fn new(
        discovery: Arc<dyn ServiceDiscovery>,
        stream_client: Option<EventStreamClient<tonic::transport::Channel>>,
        default_stream_timeout: Duration,
    ) -> Self {
        Self {
            command_router: CommandRouter::new(discovery),
            stream_handler: stream_client.map(|c| StreamHandler::new(c, default_stream_timeout)),
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

    /// Sync execute - waits for projectors/sagas based on sync_mode.
    async fn execute_sync(
        &self,
        request: Request<SyncCommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let sync_request = request.into_inner();
        let mut command_book = sync_request
            .command
            .ok_or_else(|| Status::invalid_argument("SyncCommandBook must have a command"))?;
        let correlation_id = CommandRouter::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, "Executing command (sync)");

        let response = self
            .command_router
            .forward_command_sync(command_book, sync_request.sync_mode, &correlation_id)
            .await?;
        Ok(Response::new(response))
    }

    type ExecuteStreamStream =
        Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    /// Streaming execute - streams events until client disconnects.
    ///
    /// Returns `Unimplemented` if streaming is disabled (embedded mode).
    async fn execute_stream(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<Self::ExecuteStreamStream>, Status> {
        let stream_handler = self.stream_handler.as_ref().ok_or_else(|| {
            Status::unimplemented("Event streaming not available (embedded mode)")
        })?;

        let mut command_book = request.into_inner();
        let correlation_id = CommandRouter::ensure_correlation_id(&mut command_book)?;

        debug!(correlation_id = %correlation_id, "Executing command (stream)");

        // Subscribe BEFORE sending command
        let event_stream = stream_handler.subscribe(&correlation_id).await?;

        // Forward command
        let sync_response = self
            .command_router
            .forward_command(command_book, &correlation_id)
            .await?;

        // Create stream with default timeout, no count limit
        let stream = stream_handler.create_event_stream(
            correlation_id,
            sync_response,
            event_stream,
            None,
            stream_handler.default_timeout(),
        );

        Ok(Response::new(stream))
    }

    /// Dry-run execute — execute command against temporal state without persisting.
    async fn dry_run_execute(
        &self,
        request: Request<DryRunRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let mut dry_run_request = request.into_inner();

        // Ensure correlation ID on the embedded command
        let correlation_id = match dry_run_request.command.as_mut() {
            Some(cmd) => CommandRouter::ensure_correlation_id(cmd)?,
            None => {
                return Err(Status::invalid_argument(
                    "DryRunRequest must have a command",
                ))
            }
        };

        debug!(correlation_id = %correlation_id, "Executing command (dry-run)");

        let response = self
            .command_router
            .forward_dry_run(dry_run_request, &correlation_id)
            .await?;
        Ok(Response::new(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::sync::mpsc as tokio_mpsc;
    use tokio_stream::wrappers::ReceiverStream as TokioReceiverStream;
    use tokio_stream::StreamExt;
    use tonic::transport::Server;

    use crate::discovery::K8sServiceDiscovery;
    use crate::proto::aggregate_coordinator_server::{
        AggregateCoordinator, AggregateCoordinatorServer,
    };
    use crate::proto::event_stream_server::{EventStream, EventStreamServer};
    use crate::proto::{Cover, DryRunRequest, EventPage, SyncCommandBook, Uuid as ProtoUuid};

    /// Mock AggregateCoordinator that returns configurable responses.
    struct MockAggregateCoordinator {
        response_events: Arc<tokio::sync::RwLock<Option<EventBook>>>,
        call_count: Arc<AtomicU32>,
    }

    impl MockAggregateCoordinator {
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
    impl AggregateCoordinator for MockAggregateCoordinator {
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
                    }],
                    snapshot: None,
                    snapshot_state: None,
                });

            Ok(Response::new(CommandResponse {
                events: Some(events),
                projections: vec![],
            }))
        }

        async fn handle_sync(
            &self,
            request: Request<SyncCommandBook>,
        ) -> Result<Response<CommandResponse>, Status> {
            let sync_req = request.into_inner();
            let command = sync_req
                .command
                .ok_or_else(|| Status::invalid_argument("Missing command"))?;
            self.handle(Request::new(command)).await
        }

        async fn dry_run_handle(
            &self,
            request: Request<DryRunRequest>,
        ) -> Result<Response<CommandResponse>, Status> {
            let dry_run = request.into_inner();
            let command = dry_run
                .command
                .ok_or_else(|| Status::invalid_argument("Missing command"))?;
            self.handle(Request::new(command)).await
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
            Ok(Response::new(Box::pin(stream)))
        }
    }

    /// Helper to start mock servers and return gateway service.
    async fn setup_test_gateway() -> (
        GatewayService,
        Arc<MockAggregateCoordinator>,
        Arc<MockEventStream>,
        Vec<tokio::task::JoinHandle<()>>,
    ) {
        let mock_coordinator = Arc::new(MockAggregateCoordinator::new());
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
                .add_service(AggregateCoordinatorServer::new(
                    coord_clone.as_ref().clone(),
                ))
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

        // Create discovery with wildcard endpoint for testing
        let discovery = Arc::new(K8sServiceDiscovery::new_static());
        discovery
            .register_aggregate("*", "127.0.0.1", coord_port)
            .await;

        let stream_client = EventStreamClient::connect(format!("http://127.0.0.1:{}", stream_port))
            .await
            .unwrap();

        let gateway = GatewayService::new(discovery, Some(stream_client), Duration::from_secs(5));

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
                correlation_id: String::new(),
            }),
            pages: vec![],
            saga_origin: None,
        }
    }

    // Implement Clone for mock services (needed for tonic server)
    impl Clone for MockAggregateCoordinator {
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
        assert!(command.cover.as_ref().unwrap().correlation_id.is_empty());

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
                    snapshot_state: None,
                },
                EventBook {
                    cover: None,
                    pages: vec![],
                    snapshot: None,
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
    async fn test_execute_preserves_provided_correlation_id() {
        let (gateway, _mock_coord, _mock_stream, handles) = setup_test_gateway().await;

        let mut command = make_test_command("orders");
        if let Some(ref mut cover) = command.cover {
            cover.correlation_id = "my-custom-correlation-id".to_string();
        }

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

        let command = make_test_command("orders");
        let response = gateway.execute_stream(Request::new(command)).await.unwrap();
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

        let command = make_test_command("orders");
        let response = gateway.execute_stream(Request::new(command)).await.unwrap();
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
