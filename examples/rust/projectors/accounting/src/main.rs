//! Accounting projector gRPC server.
//!
//! Receives events and writes financial data to PostgreSQL.

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use sqlx::PgPool;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
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
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<Projection>, Status> {
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
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

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

    // Check transport type
    let transport_type = env::var("TRANSPORT_TYPE").unwrap_or_else(|_| "tcp".to_string());

    if transport_type == "uds" {
        let base_path = env::var("UDS_BASE_PATH").unwrap_or_else(|_| "/tmp/angzarr".to_string());
        let service_name = env::var("SERVICE_NAME").unwrap_or_else(|_| "projector".to_string());
        let domain = env::var("DOMAIN").unwrap_or_else(|_| "accounting".to_string());
        let socket_path = PathBuf::from(format!("{}/{}-{}.sock", base_path, service_name, domain));

        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }
        let uds = UnixListener::bind(&socket_path)?;
        let uds_stream = UnixListenerStream::new(uds);

        info!(
            projector = projector_accounting::PROJECTOR_NAME,
            path = %socket_path.display(),
            transport = "uds",
            "projector_server_started"
        );

        Server::builder()
            .add_service(health_service)
            .add_service(ProjectorServer::new(service))
            .serve_with_incoming(uds_stream)
            .await?;
    } else {
        let port = env::var("PORT").unwrap_or_else(|_| "50143".to_string());
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

        info!(
            projector = projector_accounting::PROJECTOR_NAME,
            port = %port,
            transport = "tcp",
            "projector_server_started"
        );

        Server::builder()
            .add_service(health_service)
            .add_service(ProjectorServer::new(service))
            .serve(addr)
            .await?;
    }

    Ok(())
}
