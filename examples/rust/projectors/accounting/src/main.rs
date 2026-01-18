//! Accounting projector gRPC server.
//!
//! Receives events and writes financial data to PostgreSQL.

use std::env;
use std::net::SocketAddr;

use sqlx::PgPool;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

use angzarr::proto::projector_server::{Projector as ProjectorService, ProjectorServer};
use angzarr::proto::{EventBook, Projection};
use projector_accounting::AccountingProjector;

/// gRPC service implementation wrapping the projector.
struct AccountingProjectorService {
    projector: AccountingProjector,
}

impl AccountingProjectorService {
    fn new(projector: AccountingProjector) -> Self {
        Self { projector }
    }
}

#[tonic::async_trait]
impl ProjectorService for AccountingProjectorService {
    async fn handle(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<Projection>, Status> {
        let book = request.into_inner();

        match self.projector.handle(&book).await {
            Ok(Some(projection)) => Ok(Response::new(projection)),
            Ok(None) => Ok(Response::new(Projection::default())),
            Err(e) => {
                error!(error = %e, "Projection failed");
                Err(Status::internal(e.to_string()))
            }
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

    // Get database URL from environment
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let port = env::var("PORT").unwrap_or_else(|_| "50051".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    // Connect to PostgreSQL
    info!(database = %database_url, "Connecting to database");
    let pool = PgPool::connect(&database_url).await?;

    // Create and initialize projector
    let projector = AccountingProjector::new(pool);
    projector.init().await?;
    info!("Database schema initialized");

    let service = AccountingProjectorService::new(projector);

    // Create gRPC health service
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<ProjectorServer<AccountingProjectorService>>()
        .await;

    info!(
        projector = projector_accounting::PROJECTOR_NAME,
        port = %port,
        "server_started"
    );

    Server::builder()
        .add_service(health_service)
        .add_service(ProjectorServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
