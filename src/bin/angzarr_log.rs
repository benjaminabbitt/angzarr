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
//! - transport.type: "tcp" or "uds"
//! - transport.tcp.port: Port for TCP (default: 50051)
//! - transport.uds.base_path: Base path for UDS sockets
//! - DESCRIPTOR_PATH: Path to FileDescriptorSet for JSON decoding (optional)
//!
//! If DESCRIPTOR_PATH is set, events are decoded to JSON using prost-reflect.
//! Otherwise, events are displayed as hex dumps with type information.

use std::sync::Arc;

use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tracing::{error, info};

use angzarr::config::Config;
use angzarr::handlers::projectors::log::{LogService, LogServiceHandle};
use angzarr::proto::projector_coordinator_server::ProjectorCoordinatorServer;
use angzarr::transport::{grpc_trace_layer, serve_with_transport};
use angzarr::utils::bootstrap::init_tracing;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config_path = angzarr::utils::bootstrap::parse_config_path();
    let config = Config::load(config_path.as_deref()).map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    // Create log service
    let log_service = Arc::new(LogService::new());
    let projector_service = LogServiceHandle(Arc::clone(&log_service));

    // Health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<ProjectorCoordinatorServer<LogServiceHandle>>()
        .await;

    info!("angzarr-log started");

    let router = Server::builder()
        .layer(grpc_trace_layer())
        .add_service(health_service)
        .add_service(ProjectorCoordinatorServer::new(projector_service));

    serve_with_transport(router, &config.transport, "log", None).await?;

    Ok(())
}
