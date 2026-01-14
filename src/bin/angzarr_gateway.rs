//! angzarr-gateway: Command gateway service
//!
//! Central infrastructure service that routes commands and streams back
//! resulting events.
//!
//! ## Architecture
//! ```text
//! [Client] -> [angzarr-gateway] -> [angzarr-command] -> [Business Logic]
//!    ^              |                    |
//!    |              |                    v
//!    |              |               [AMQP Events]
//!    |              |                    |
//!    |              |                    v
//!    |              +---------> [angzarr-stream]
//!    |                                   |
//!    +-----------------------------------+
//!           streams matching events back
//! ```
//!
//! ## Configuration
//! - COMMAND_ADDRESS: angzarr-command service address (e.g., "angzarr-customer:1313")
//! - STREAM_ADDRESS: angzarr-stream service address (e.g., "angzarr-stream:1315")
//! - GRPC_PORT: Port for CommandGateway gRPC service (default: 1316)
//! - STREAM_TIMEOUT_SECS: Timeout for event stream (default: 30)

use std::net::SocketAddr;
use std::time::Duration;

use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use angzarr::handlers::gateway::GatewayService;
use angzarr::proto::business_coordinator_client::BusinessCoordinatorClient;
use angzarr::proto::command_gateway_server::CommandGatewayServer;
use angzarr::proto::event_stream_client::EventStreamClient;

const DEFAULT_GRPC_PORT: u16 = 1316;
const DEFAULT_STREAM_TIMEOUT_SECS: u64 = 30;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_env("ANGZARR_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting angzarr-gateway service");

    let command_address = std::env::var("COMMAND_ADDRESS")
        .map_err(|_| "COMMAND_ADDRESS environment variable required")?;

    let stream_address = std::env::var("STREAM_ADDRESS")
        .map_err(|_| "STREAM_ADDRESS environment variable required")?;

    let stream_timeout_secs: u64 = std::env::var("STREAM_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_STREAM_TIMEOUT_SECS);

    info!("Connecting to command service at {}", command_address);
    let command_client =
        BusinessCoordinatorClient::connect(format!("http://{}", command_address)).await?;

    info!("Connecting to stream service at {}", stream_address);
    let stream_client = EventStreamClient::connect(format!("http://{}", stream_address)).await?;

    let gateway_service = GatewayService::new(
        command_client,
        stream_client,
        Duration::from_secs(stream_timeout_secs),
    );

    let grpc_port = std::env::var("GRPC_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_GRPC_PORT);

    let addr: SocketAddr = format!("0.0.0.0:{}", grpc_port).parse()?;

    info!(
        "CommandGateway gRPC server listening on {} (stream timeout: {}s)",
        addr, stream_timeout_secs
    );

    // Create health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;

    Server::builder()
        .add_service(health_service)
        .add_service(CommandGatewayServer::new(gateway_service))
        .serve(addr)
        .await?;

    Ok(())
}
