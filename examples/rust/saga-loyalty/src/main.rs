//! Loyalty Points Saga gRPC server.
//!
//! Listens to TransactionCompleted events and sends AddLoyaltyPoints
//! commands to the customer domain.

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use evented::interfaces::saga::Saga as SagaTrait;
use evented::proto::saga_coordinator_server::{SagaCoordinator, SagaCoordinatorServer};
use evented::proto::{EventBook, SynchronousProcessingResponse};
use saga_loyalty::LoyaltyPointsSaga;

const SAGA_NAME: &str = "loyalty_points";

/// gRPC service implementation wrapping the saga logic.
struct LoyaltyService {
    saga: LoyaltyPointsSaga,
}

impl LoyaltyService {
    fn new() -> Self {
        Self {
            saga: LoyaltyPointsSaga::new(),
        }
    }
}

#[tonic::async_trait]
impl SagaCoordinator for LoyaltyService {
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        let event_book = Arc::new(request.into_inner());
        let _ = self.saga.handle(&event_book).await;
        Ok(Response::new(()))
    }

    async fn handle_sync(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<SynchronousProcessingResponse>, Status> {
        let event_book = Arc::new(request.into_inner());

        match self.saga.handle(&event_book).await {
            Ok(commands) => Ok(Response::new(SynchronousProcessingResponse {
                books: vec![],
                commands,
                projections: vec![],
            })),
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

    let port = env::var("PORT").unwrap_or_else(|_| "50054".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    let service = LoyaltyService::new();

    info!(
        saga = SAGA_NAME,
        port = %port,
        listens_to = "transaction domain",
        "server_started"
    );

    Server::builder()
        .add_service(SagaCoordinatorServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
