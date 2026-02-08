//! Tests for gRPC communication over Unix Domain Sockets (UDS).

use crate::common::*;
use angzarr::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use angzarr::proto::aggregate_coordinator_server::{
    AggregateCoordinator, AggregateCoordinatorServer,
};
use angzarr::proto::{CommandResponse, SyncCommandBook};
use angzarr::transport::{connect_to_address, prepare_uds_socket};
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response};

/// Mock aggregate service for UDS tests.
struct MockAggregateService {
    call_count: AtomicU32,
}

impl MockAggregateService {
    fn new() -> Self {
        Self {
            call_count: AtomicU32::new(0),
        }
    }
}

#[tonic::async_trait]
impl AggregateCoordinator for MockAggregateService {
    async fn handle(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let cmd = request.into_inner();

        // Echo command as event
        let events = EventBook {
            cover: cmd.cover,
            pages: vec![EventPage {
                sequence: Some(event_page::Sequence::Num(0)),
                event: cmd.pages.first().and_then(|p| p.command.clone()),
                created_at: None,
            }],
            snapshot: None,
        };

        Ok(Response::new(CommandResponse {
            events: Some(events),
            projections: Vec::new(),
        }))
    }

    async fn handle_sync(
        &self,
        request: Request<SyncCommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let sync_cmd = request.into_inner();
        let cmd = sync_cmd.command.unwrap_or_default();

        self.call_count.fetch_add(1, Ordering::SeqCst);

        let events = EventBook {
            cover: cmd.cover,
            pages: vec![EventPage {
                sequence: Some(event_page::Sequence::Num(0)),
                event: cmd.pages.first().and_then(|p| p.command.clone()),
                created_at: None,
            }],
            snapshot: None,
        };

        Ok(Response::new(CommandResponse {
            events: Some(events),
            projections: Vec::new(),
        }))
    }

    async fn dry_run_handle(
        &self,
        request: Request<angzarr::proto::DryRunRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let dry_run = request.into_inner();
        let cmd = dry_run.command.unwrap_or_default();
        self.handle(Request::new(cmd)).await
    }
}

#[tokio::test]
async fn test_grpc_server_and_client_over_uds() {
    let base_path = temp_dir();
    let socket_path = base_path.join("test-aggregate.sock");

    // Start gRPC server on UDS
    let _guard = prepare_uds_socket(&socket_path).expect("Failed to prepare socket");
    let uds = UnixListener::bind(&socket_path).expect("Failed to bind UDS");
    let uds_stream = UnixListenerStream::new(uds);

    let service = MockAggregateService::new();
    let server = Server::builder().add_service(AggregateCoordinatorServer::new(service));

    // Run server in background
    let server_task = tokio::spawn(async move {
        server.serve_with_incoming(uds_stream).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect client via UDS
    let channel = connect_to_address(socket_path.to_str().unwrap())
        .await
        .expect("Failed to connect");
    let mut client = AggregateCoordinatorClient::new(channel);

    // Execute command
    let command = create_test_command("orders", Uuid::new_v4(), b"test-data", 0);
    let response = client.handle(command).await.expect("RPC failed");
    let sync_resp = response.into_inner();

    assert!(sync_resp.events.is_some(), "Should return events");
    assert_eq!(
        sync_resp.events.as_ref().unwrap().pages.len(),
        1,
        "Should have one event"
    );

    server_task.abort();
    cleanup_dir(&base_path);
}

#[tokio::test]
async fn test_multiple_concurrent_uds_requests() {
    let base_path = temp_dir();
    let socket_path = base_path.join("concurrent-aggregate.sock");

    let _guard = prepare_uds_socket(&socket_path).expect("Failed to prepare socket");
    let uds = UnixListener::bind(&socket_path).expect("Failed to bind UDS");
    let uds_stream = UnixListenerStream::new(uds);

    let service = MockAggregateService::new();
    let server = Server::builder().add_service(AggregateCoordinatorServer::new(service));

    let server_task = tokio::spawn(async move {
        server.serve_with_incoming(uds_stream).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Create multiple clients
    let mut handles = Vec::new();
    for i in 0..10 {
        let path = socket_path.clone();
        let handle = tokio::spawn(async move {
            let channel = connect_to_address(path.to_str().unwrap())
                .await
                .expect("Failed to connect");
            let mut client = AggregateCoordinatorClient::new(channel);

            let command = create_test_command("orders", Uuid::new_v4(), format!("request-{}", i).as_bytes(), 0);
            client.handle(command).await.expect("RPC failed")
        });
        handles.push(handle);
    }

    // All requests should succeed
    for handle in handles {
        let response = handle.await.expect("Task panicked");
        assert!(response.into_inner().events.is_some());
    }

    server_task.abort();
    cleanup_dir(&base_path);
}

#[tokio::test]
async fn test_uds_socket_cleanup_on_server_restart() {
    let base_path = temp_dir();
    let socket_path = base_path.join("restart-aggregate.sock");

    // First server instance
    {
        let _guard = prepare_uds_socket(&socket_path).expect("Failed to prepare socket");
        let uds = UnixListener::bind(&socket_path).expect("Failed to bind UDS");
        let uds_stream = UnixListenerStream::new(uds);

        let service = MockAggregateService::new();
        let server = Server::builder().add_service(AggregateCoordinatorServer::new(service));

        let server_task = tokio::spawn(async move {
            server.serve_with_incoming(uds_stream).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        server_task.abort();
    }

    // Socket file may still exist - prepare_uds_socket should clean it up
    let _guard =
        prepare_uds_socket(&socket_path).expect("Should be able to prepare socket again");
    let uds = UnixListener::bind(&socket_path).expect("Should be able to bind again");
    let uds_stream = UnixListenerStream::new(uds);

    let service = MockAggregateService::new();
    let server = Server::builder().add_service(AggregateCoordinatorServer::new(service));

    let server_task = tokio::spawn(async move {
        server.serve_with_incoming(uds_stream).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Should be able to connect
    let channel = connect_to_address(socket_path.to_str().unwrap())
        .await
        .expect("Failed to connect to restarted server");
    let mut client = AggregateCoordinatorClient::new(channel);

    let command = create_test_command("orders", Uuid::new_v4(), b"after-restart", 0);
    let response = client
        .handle(command)
        .await
        .expect("RPC to restarted server failed");
    assert!(response.into_inner().events.is_some());

    server_task.abort();
    cleanup_dir(&base_path);
}
