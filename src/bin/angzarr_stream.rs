//! angzarr-stream: Event streaming and CloudEvents projector
//!
//! Infrastructure projector that streams events to internal and external consumers.
//! Receives events from the projector sidecar (via Projector gRPC) and forwards
//! to gateway subscribers (gRPC) and CloudEvents sinks (HTTP/Kafka).
//!
//! ## Architecture
//! ```text
//! [angzarr-projector sidecar] --(Projector gRPC)--> [angzarr-stream]
//!                                                         |
//!                                              ┌──────────┴──────────┐
//!                                              ▼                     ▼
//!                                    [EventStream gRPC]    [CloudEvents Sinks]
//!                                          |                    |
//!                                          ▼                    ▼
//!                                  [angzarr-gateway]     [HTTP/Kafka endpoints]
//! ```
//!
//! Unlike client logic projectors, the stream projector is core infrastructure
//! that enables real-time event streaming to connected clients via the gateway
//! and external systems via CloudEvents.
//!
//! ## Configuration
//! - transport.type: "tcp" or "uds"
//! - transport.tcp.port: Port for TCP (default: 50051)
//! - transport.uds.base_path: Base path for UDS sockets
//! - CLOUDEVENTS_SINK: `http`, `kafka`, `both`, or `null` (default: null)
//! - OUTBOUND_CONTENT_TYPE: `json` or `protobuf` (default: json)
//! - CLOUDEVENTS_HTTP_ENDPOINT: HTTP webhook URL (if using http sink)
//! - CLOUDEVENTS_KAFKA_BROKERS: Kafka brokers (if using kafka sink)

use std::sync::Arc;

use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tonic_health::server::health_reporter;
use tracing::{error, info, warn};

use angzarr::config::Config;
use angzarr::handlers::projectors::outbound::{self, OutboundService};
use angzarr::proto::event_stream_service_server::EventStreamServiceServer;
use angzarr::proto::projector_coordinator_service_server::ProjectorCoordinatorServiceServer;
use angzarr::proto::{EventBook, Projection, SpeculateProjectorRequest, SyncEventBook};
use angzarr::transport::{grpc_trace_layer, serve_with_transport};
use angzarr::utils::bootstrap::init_tracing;

/// Projector service that receives events from the projector sidecar.
/// Implements ProjectorCoordinator (HandleSync) which is what angzarr-projector calls.
struct OutboundProjectorService {
    outbound_service: Arc<OutboundService>,
}

impl OutboundProjectorService {
    fn new(outbound_service: Arc<OutboundService>) -> Self {
        Self { outbound_service }
    }
}

#[tonic::async_trait]
impl angzarr::proto::projector_coordinator_service_server::ProjectorCoordinatorService
    for OutboundProjectorService
{
    /// Handle sync event book from projector sidecar.
    async fn handle_sync(
        &self,
        request: Request<SyncEventBook>,
    ) -> Result<Response<Projection>, Status> {
        let sync_book = request.into_inner();
        if let Some(book) = sync_book.events {
            if let Err(e) = self.outbound_service.handle(&book).await {
                warn!(error = %e, "OutboundService handle failed");
            }
        }

        // Outbound projector doesn't produce projection output
        Ok(Response::new(Projection::default()))
    }

    /// Fire-and-forget handle (not used by projector sidecar).
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        let book = request.into_inner();
        if let Err(e) = self.outbound_service.handle(&book).await {
            warn!(error = %e, "OutboundService handle failed");
        }
        Ok(Response::new(()))
    }

    /// Speculative handle - outbound projector doesn't produce projections.
    async fn handle_speculative(
        &self,
        _request: Request<SpeculateProjectorRequest>,
    ) -> Result<Response<Projection>, Status> {
        // Outbound projector doesn't produce projection output
        // Speculative execution is a no-op for streaming
        Ok(Response::new(Projection::default()))
    }
}

/// Wrapper to implement EventStream for Arc<OutboundService>.
#[derive(Clone)]
struct OutboundServiceWrapper(Arc<OutboundService>);

impl std::ops::Deref for OutboundServiceWrapper {
    type Target = OutboundService;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[tonic::async_trait]
impl angzarr::proto::event_stream_service_server::EventStreamService for OutboundServiceWrapper {
    type SubscribeStream =
        <OutboundService as angzarr::proto::event_stream_service_server::EventStreamService>::SubscribeStream;

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

    // Create outbound service with sinks from environment
    let outbound_service = Arc::new(match outbound::from_env() {
        Ok(service) => {
            info!("OutboundService configured with CloudEvents sinks");
            service
        }
        Err(e) => {
            // If sink config fails, fall back to gRPC-only mode
            warn!(
                error = %e,
                "CloudEvents sink configuration failed, running in gRPC-only mode"
            );
            OutboundService::new()
        }
    });

    // Projector service receives events from sidecar
    let projector_service = OutboundProjectorService::new(Arc::clone(&outbound_service));

    // EventStream service for gateway subscriptions
    let event_stream_service = OutboundServiceWrapper(Arc::clone(&outbound_service));

    // Health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<ProjectorCoordinatorServiceServer<OutboundProjectorService>>()
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
