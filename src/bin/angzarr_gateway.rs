//! angzarr-gateway: Command gateway service
//!
//! Central infrastructure service that routes commands to domain-specific
//! handlers and streams back resulting events.
//!
//! ## Architecture
//! ```text
//! [Client] -> [angzarr-gateway] -> [angzarr-aggregate-*] -> [Business Logic]
//!    ^              |                      |
//!    |              |    (domain routing)  v
//!    |              |                 [AMQP Events]
//!    |              |                      |
//!    |              |                      v
//!    |              +--------> [angzarr-stream]
//!    |                                     |
//!    +-------------------------------------+
//!             streams matching events back
//! ```
//!
//! ## Configuration
//!
//! In K8s: Auto-discovers services with labels:
//! - `app.kubernetes.io/component=aggregate`
//! - `angzarr.io/domain=<domain-name>`
//!
//! Required:
//! - STREAM_ADDRESS: angzarr-stream service address (e.g., "angzarr-stream:1315")
//!
//! Optional:
//! - ANGZARR_PORT: Port for all services - gRPC over HTTP/2 (default: 50051)
//! - STREAM_TIMEOUT_SECS: Timeout for event stream (default: 30)
//! - NAMESPACE: Kubernetes namespace for service discovery (default: from downward API)

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use angzarr::discovery::ServiceDiscovery;
use angzarr::handlers::gateway::{EventQueryProxy, GatewayService};
use angzarr::proto::command_gateway_server::CommandGatewayServer;
use angzarr::proto::event_query_server::EventQueryServer;
use angzarr::proto::event_stream_client::EventStreamClient;

/// Standard Angzarr port - gRPC over HTTP/2, used across all languages/containers
const DEFAULT_PORT: u16 = 50051;
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

    // Initialize service discovery using K8s labels
    let discovery = match ServiceDiscovery::from_env().await {
        Ok(discovery) => {
            let discovery = Arc::new(discovery);

            // Perform initial sync and start watching for changes
            if let Err(e) = discovery.initial_sync().await {
                error!("Service discovery initial sync failed: {}", e);
                return Err(e.into());
            }

            discovery.start_watching();
            info!("K8s service discovery initialized");
            discovery
        }
        Err(e) => {
            error!("Failed to initialize service discovery: {}", e);
            return Err(format!("Service discovery required: {}", e).into());
        }
    };

    let domains = discovery.aggregate_domains().await;
    info!(
        domains = ?domains,
        "Service discovery found {} aggregate(s)",
        domains.len()
    );

    // Stream service connection (centralized, not domain-specific)
    let stream_address = std::env::var("STREAM_ADDRESS")
        .map_err(|_| "STREAM_ADDRESS environment variable required")?;

    let stream_timeout_secs: u64 = std::env::var("STREAM_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_STREAM_TIMEOUT_SECS);

    info!("Connecting to stream service at {}", stream_address);
    let stream_url = format!("http://{}", stream_address);
    let stream_client = {
        let max_retries = 30;
        let mut delay = Duration::from_millis(100);
        let mut attempt = 0;
        loop {
            attempt += 1;
            match EventStreamClient::connect(stream_url.clone()).await {
                Ok(client) => {
                    info!("Connected to stream service at {}", stream_address);
                    break client;
                }
                Err(e) if attempt < max_retries => {
                    warn!(
                        "Failed to connect to stream (attempt {}/{}): {}. Retrying in {:?}...",
                        attempt, max_retries, e, delay
                    );
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay * 2, Duration::from_secs(5));
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to connect to stream after {} attempts: {}",
                        max_retries,
                        e
                    );
                    return Err(e.into());
                }
            }
        }
    };

    let gateway_service = GatewayService::new(
        discovery.clone(),
        stream_client,
        Duration::from_secs(stream_timeout_secs),
    );

    // EventQuery proxy routes queries to appropriate aggregate sidecars by domain
    let event_query_proxy = EventQueryProxy::new(discovery);

    let port = std::env::var("ANGZARR_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    info!(
        "Angzarr gateway listening on port {} (stream timeout: {}s)",
        port, stream_timeout_secs
    );

    // Create health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;

    Server::builder()
        .add_service(health_service)
        .add_service(CommandGatewayServer::new(gateway_service))
        .add_service(EventQueryServer::new(event_query_proxy))
        .serve(addr)
        .await?;

    Ok(())
}
