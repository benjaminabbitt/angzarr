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
//! Unlike business logic projectors, the stream projector is core infrastructure
//! that enables real-time event streaming to connected clients via the gateway.
//!
//! ## Configuration
//! - PORT: Port for gRPC services (default: 50051)

use std::net::SocketAddr;
use std::sync::Arc;

use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tonic_health::server::health_reporter;
use tracing::info;

use angzarr::utils::bootstrap::init_tracing;

use angzarr::handlers::stream::StreamService;
use angzarr::proto::event_stream_server::EventStreamServer;
use angzarr::proto::projector_server::{Projector, ProjectorServer};
use angzarr::proto::{EventBook, Projection};

const DEFAULT_PORT: u16 = 50051;

/// Projector service that receives events from the projector sidecar.
struct StreamProjectorService {
    stream_service: Arc<StreamService>,
}

impl StreamProjectorService {
    fn new(stream_service: Arc<StreamService>) -> Self {
        Self { stream_service }
    }
}

#[tonic::async_trait]
impl Projector for StreamProjectorService {
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<Projection>, Status> {
        let book = request.into_inner();
        self.stream_service.handle(&book).await;

        // Stream projector doesn't produce projection output
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
impl angzarr::proto::event_stream_server::EventStream for StreamServiceWrapper {
    type SubscribeStream = <StreamService as angzarr::proto::event_stream_server::EventStream>::SubscribeStream;

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

    let port = std::env::var("ANGZARR_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    // Create shared stream service
    let stream_service = Arc::new(StreamService::new());

    // Projector service receives events from sidecar
    let projector_service = StreamProjectorService::new(Arc::clone(&stream_service));

    // EventStream service for gateway subscriptions
    let event_stream_service = StreamServiceWrapper(Arc::clone(&stream_service));

    // Health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<ProjectorServer<StreamProjectorService>>()
        .await;

    info!(port = %port, "angzarr-stream started");

    Server::builder()
        .add_service(health_service)
        .add_service(ProjectorServer::new(projector_service))
        .add_service(EventStreamServer::new(event_stream_service))
        .serve(addr)
        .await?;

    Ok(())
}
