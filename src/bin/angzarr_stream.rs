//! angzarr-stream: Event streaming projector
//!
//! Infrastructure projector that streams events to registered subscribers.
//! Receives events from the projector sidecar (via Projector gRPC) and forwards
//! to gateway subscribers filtered by correlation ID.
//!
//! ## Architecture
//! ```text
//! [angzarr-projector sidecar] --(Projector gRPC)--> [angzarr-stream]
//!                                                         |
//!                                                         v
//!                                               (correlation ID match)
//!                                                         |
//!                                                         v
//!                                               [EventStream gRPC] --> [angzarr-gateway]
//! ```
//!
//! Unlike client logic projectors, the stream projector is core infrastructure
//! that enables real-time event streaming to connected clients via the gateway.
//!
//! ## Configuration
//! - transport.type: "tcp" or "uds"
//! - transport.tcp.port: Port for TCP (default: 50051)
//! - transport.uds.base_path: Base path for UDS sockets

use std::sync::Arc;

use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tonic_health::server::health_reporter;
use tracing::{error, info};

use angzarr::config::Config;
use angzarr::handlers::projectors::stream::StreamService;
use angzarr::proto::event_stream_service_server::EventStreamServiceServer;
use angzarr::proto::projector_coordinator_service_server::{
    ProjectorCoordinatorService, ProjectorCoordinatorServiceServer,
};
use angzarr::proto::{EventBook, Projection, SpeculateProjectorRequest, SyncEventBook};
use angzarr::transport::{grpc_trace_layer, serve_with_transport};
use angzarr::utils::bootstrap::init_tracing;

/// Projector service that receives events from the projector sidecar.
/// Implements ProjectorCoordinator (HandleSync) which is what angzarr-projector calls.
struct StreamProjectorService {
    stream_service: Arc<StreamService>,
}

impl StreamProjectorService {
    fn new(stream_service: Arc<StreamService>) -> Self {
        Self { stream_service }
    }
}

#[tonic::async_trait]
impl angzarr::proto::projector_coordinator_service_server::ProjectorCoordinatorService
    for StreamProjectorService
{
    /// Handle sync event book from projector sidecar.
    async fn handle_sync(
        &self,
        request: Request<SyncEventBook>,
    ) -> Result<Response<Projection>, Status> {
        let sync_book = request.into_inner();
        if let Some(book) = sync_book.events {
            self.stream_service.handle(&book).await;
        }

        // Stream projector doesn't produce projection output
        Ok(Response::new(Projection::default()))
    }

    /// Fire-and-forget handle (not used by projector sidecar).
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        let book = request.into_inner();
        self.stream_service.handle(&book).await;
        Ok(Response::new(()))
    }

    /// Speculative handle - stream projector doesn't produce projections.
    async fn handle_speculative(
        &self,
        _request: Request<SpeculateProjectorRequest>,
    ) -> Result<Response<Projection>, Status> {
        // Stream projector doesn't produce projection output
        // Speculative execution is a no-op for streaming
        Ok(Response::new(Projection::default()))
    }
}

/// Wrapper to implement EventStream for Arc<StreamService>.
#[derive(Clone)]
struct StreamServiceWrapper(Arc<StreamService>);

impl std::ops::Deref for StreamServiceWrapper {
    type Target = StreamService;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[tonic::async_trait]
impl angzarr::proto::event_stream_service_server::EventStreamService for StreamServiceWrapper {
    type SubscribeStream =
        <StreamService as angzarr::proto::event_stream_service_server::EventStreamService>::SubscribeStream;

    async fn subscribe(
        &self,
        request: Request<angzarr::proto::EventStreamFilter>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        self.0.subscribe(request).await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config_path = angzarr::utils::bootstrap::parse_config_path();
    let config = Config::load(config_path.as_deref()).map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    // Create shared stream service
    let stream_service = Arc::new(StreamService::new());

    // Projector service receives events from sidecar
    let projector_service = StreamProjectorService::new(Arc::clone(&stream_service));

    // EventStream service for gateway subscriptions
    let event_stream_service = StreamServiceWrapper(Arc::clone(&stream_service));

    // Health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<ProjectorCoordinatorServiceServer<StreamProjectorService>>()
        .await;

    info!("angzarr-stream started");

    let router = Server::builder()
        .layer(grpc_trace_layer())
        .add_service(health_service)
        .add_service(ProjectorCoordinatorServiceServer::new(projector_service))
        .add_service(EventStreamServiceServer::new(event_stream_service));

    serve_with_transport(router, &config.transport, "stream", None).await?;

    Ok(())
}
