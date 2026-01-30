//! Angzarr Demonstrator Event Projector.
//!
//! Writes all events as JSON to a database for querying and debugging.
//! Useful for observing event flow and debugging event-sourced systems.

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use angzarr::handlers::projectors::{connect_pool, EventService, EventServiceHandle};
use angzarr::proto::projector_coordinator_server::ProjectorCoordinatorServer;
use angzarr::transport::grpc_trace_layer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    // Get database URL from environment
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Connect to database
    info!(database = %database_url, "Connecting to database");
    let pool = connect_pool(&database_url).await?;

    // Create service, optionally with descriptors
    let mut service = EventService::new(pool);
    if let Ok(path) = env::var("DESCRIPTOR_PATH") {
        service = service.load_descriptors(&path);
    }

    // Initialize schema
    service.init().await?;
    info!("Events table schema initialized");

    let handle = EventServiceHandle(Arc::new(service));

    // Create gRPC health service
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<ProjectorCoordinatorServer<EventServiceHandle>>()
        .await;

    // Check transport type
    let transport_type = env::var("TRANSPORT_TYPE").unwrap_or_else(|_| "tcp".to_string());

    if transport_type == "uds" {
        let base_path = env::var("UDS_BASE_PATH").unwrap_or_else(|_| "/tmp/angzarr".to_string());
        let socket_path = PathBuf::from(format!("{}/projector-event.sock", base_path));

        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }
        let uds = UnixListener::bind(&socket_path)?;
        let uds_stream = UnixListenerStream::new(uds);

        info!(
            projector = "event",
            path = %socket_path.display(),
            transport = "uds",
            "projector_server_started"
        );

        Server::builder()
            .layer(grpc_trace_layer())
            .add_service(health_service)
            .add_service(ProjectorCoordinatorServer::new(handle))
            .serve_with_incoming(uds_stream)
            .await?;
    } else {
        let port = env::var("PORT").unwrap_or_else(|_| "50160".to_string());
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

        info!(
            projector = "event",
            port = %port,
            transport = "tcp",
            "projector_server_started"
        );

        Server::builder()
            .layer(grpc_trace_layer())
            .add_service(health_service)
            .add_service(ProjectorCoordinatorServer::new(handle))
            .serve(addr)
            .await?;
    }

    Ok(())
}
