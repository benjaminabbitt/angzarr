//! Receipt Projector gRPC server.
//!
//! Generates human-readable receipts when transactions complete.

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use evented::interfaces::projector::Projector as ProjectorTrait;
use evented::proto::projector_coordinator_server::{ProjectorCoordinator, ProjectorCoordinatorServer};
use evented::proto::{EventBook, Projection};
use projector_receipt::ReceiptProjector;

const PROJECTOR_NAME: &str = "receipt";

/// gRPC service implementation wrapping the projector logic.
struct ReceiptService {
    projector: ReceiptProjector,
}

impl ReceiptService {
    fn new() -> Self {
        Self {
            projector: ReceiptProjector::new(),
        }
    }
}

#[tonic::async_trait]
impl ProjectorCoordinator for ReceiptService {
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
            Ok(None) => Err(Status::not_found("No projection generated")),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json().init();

    let port = env::var("PORT").unwrap_or_else(|_| "50055".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    let service = ReceiptService::new();

    info!(
        projector = PROJECTOR_NAME,
        port = %port,
        listens_to = "transaction domain",
        "server_started"
    );

    Server::builder()
        .add_service(ProjectorCoordinatorServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
