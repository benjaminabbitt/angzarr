//! Order Cancellation Saga gRPC server.
//!
//! Handles compensation when orders are cancelled.

use std::env;
use std::net::SocketAddr;

use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use angzarr::proto::saga_server::{Saga as SagaService, SagaServer};
use angzarr::proto::{EventBook, SagaResponse};
use saga_cancellation::CancellationSaga;

/// gRPC service implementation wrapping the saga.
struct CancellationSagaService {
    saga: CancellationSaga,
}

impl CancellationSagaService {
    fn new() -> Self {
        Self {
            saga: CancellationSaga::new(),
        }
    }
}

#[tonic::async_trait]
impl SagaService for CancellationSagaService {
    async fn handle(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<SagaResponse>, Status> {
        let event_book = request.into_inner();
        let commands = self.saga.handle(&event_book);
        Ok(Response::new(SagaResponse { commands }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json()
        .init();

    let port = env::var("PORT").unwrap_or_else(|_| "50062".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    let service = CancellationSagaService::new();

    // Create gRPC health service
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<SagaServer<CancellationSagaService>>()
        .await;

    info!(
        saga = saga_cancellation::SAGA_NAME,
        port = %port,
        "saga_server_started"
    );

    Server::builder()
        .add_service(health_service)
        .add_service(SagaServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
