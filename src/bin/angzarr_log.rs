//! angzarr-log: Logging projector
//!
//! Infrastructure projector that pretty-prints events to stdout.
//! Useful for debugging and monitoring event flow.
//!
//! ## Architecture
//! ```text
//! [angzarr-projector sidecar] --(Projector gRPC)--> [angzarr-log]
//!                                                         |
//!                                                         v
//!                                                      stdout
//! ```
//!
//! ## Configuration
//! - ANGZARR_PORT: Port for gRPC services (default: 50051)
//! - DESCRIPTOR_PATH: Path to FileDescriptorSet for JSON decoding (optional)
//!
//! If DESCRIPTOR_PATH is set, events are decoded to JSON using prost-reflect.
//! Otherwise, events are displayed as hex dumps with type information.

use std::net::SocketAddr;
use std::sync::Arc;

use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tracing::info;

use angzarr::handlers::projectors::log::{LogService, LogServiceHandle};
use angzarr::proto::projector_coordinator_server::ProjectorCoordinatorServer;
use angzarr::utils::bootstrap::init_tracing;

const DEFAULT_PORT: u16 = 50051;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let port = std::env::var("ANGZARR_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    // Create log service
    let log_service = Arc::new(LogService::new());
    let projector_service = LogServiceHandle(Arc::clone(&log_service));

    // Health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<ProjectorCoordinatorServer<LogServiceHandle>>()
        .await;

    info!(port = %port, "angzarr-log started");

    Server::builder()
        .add_service(health_service)
        .add_service(ProjectorCoordinatorServer::new(projector_service))
        .serve(addr)
        .await?;

    Ok(())
}
