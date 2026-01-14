//! Customer Log Projector gRPC server.
//!
//! Logs customer events using structured logging.

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use angzarr::interfaces::projector::Projector as ProjectorTrait;
use angzarr::proto::projector_coordinator_server::{
    ProjectorCoordinator, ProjectorCoordinatorServer,
};
use angzarr::proto::{EventBook, Projection};
use projector_log_customer::CustomerLogProjector;

const PROJECTOR_NAME: &str = "log-customer";

/// gRPC service implementation wrapping the projector logic.
struct LogCustomerService {
    projector: CustomerLogProjector,
}

impl LogCustomerService {
    fn new() -> Self {
        Self {
            projector: CustomerLogProjector::new(),
        }
    }
}

#[tonic::async_trait]
impl ProjectorCoordinator for LogCustomerService {
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        let event_book = Arc::new(request.into_inner());
        let _ = self.projector.project(&event_book).await;
        Ok(Response::new(()))
    }

    async fn handle_sync(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<Projection>, Status> {
        let event_book = Arc::new(request.into_inner());

        match self.projector.project(&event_book).await {
            Ok(Some(projection)) => Ok(Response::new(projection)),
            Ok(None) => Err(Status::not_found(
                "Log projector does not produce projections",
            )),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json()
        .init();

    let port = env::var("PORT").unwrap_or_else(|_| "50056".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    let service = LogCustomerService::new();

    // Create gRPC health service
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<ProjectorCoordinatorServer<LogCustomerService>>()
        .await;

    info!(
        projector = PROJECTOR_NAME,
        port = %port,
        listens_to = "customer domain",
        "server_started"
    );

    Server::builder()
        .add_service(health_service)
        .add_service(ProjectorCoordinatorServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
