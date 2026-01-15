//! angzarr-gateway: Command gateway service
//!
//! Central infrastructure service that routes commands to domain-specific
//! handlers and streams back resulting events.
//!
//! ## Architecture
//! ```text
//! [Client] -> [angzarr-gateway] -> [angzarr-entity-*] -> [Business Logic]
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
//! Command service discovery (auto-detected):
//! - In K8s: Auto-discovers services with label `app.kubernetes.io/component=business`
//!   and annotation `angzarr.io/domain=<domain-name>`
//! - Outside K8s: Uses environment variables:
//!   - COMMAND_HANDLERS: Multi-domain format "domain1=addr:port,domain2=addr:port"
//!   - COMMAND_ADDRESS: Single address for wildcard routing (legacy)
//!
//! Required:
//! - STREAM_ADDRESS: angzarr-stream service address (e.g., "angzarr-stream:1315")
//!
//! Optional:
//! - GRPC_PORT: Port for CommandGateway gRPC service (default: 1316)
//! - STREAM_TIMEOUT_SECS: Timeout for event stream (default: 30)
//! - NAMESPACE: Kubernetes namespace for service discovery (default: from downward API)

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use angzarr::discovery::{k8s::K8sServiceDiscovery, static_config, ServiceRegistry};
use angzarr::handlers::gateway::{EventQueryProxy, GatewayService};
use angzarr::proto::command_gateway_server::CommandGatewayServer;
use angzarr::proto::event_query_server::EventQueryServer;
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

    // Initialize service registry for domain-based command routing
    let registry = Arc::new(ServiceRegistry::new());

    // Choose discovery method based on environment
    if std::env::var("KUBERNETES_SERVICE_HOST").is_ok() {
        // Running in Kubernetes - use API-based discovery
        info!("Detected Kubernetes environment, using API-based discovery");

        let discovery = K8sServiceDiscovery::new(registry.clone())
            .await
            .map_err(|e| format!("Failed to initialize K8s discovery: {}", e))?;

        discovery
            .initial_sync()
            .await
            .map_err(|e| format!("Failed to sync services from K8s: {}", e))?;

        // Start background watcher for service changes
        discovery.start_watching();

        info!("K8s service discovery active");
    } else {
        // Running outside K8s - use static config from environment
        info!("No Kubernetes detected, using static configuration");

        static_config::load_from_env(registry.clone())
            .await
            .map_err(|e| format!("Failed to load service configuration: {}", e))?;
    }

    let domains = registry.domains().await;
    info!(
        domains = ?domains,
        "Service registry initialized with {} endpoint(s)",
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
        registry.clone(),
        stream_client,
        Duration::from_secs(stream_timeout_secs),
    );

    // EventQuery proxy routes queries to appropriate entity sidecars by domain
    let event_query_proxy = EventQueryProxy::new(registry);

    let grpc_port = std::env::var("GRPC_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_GRPC_PORT);

    let addr: SocketAddr = format!("0.0.0.0:{}", grpc_port).parse()?;

    info!(
        "CommandGateway + EventQuery gRPC server listening on {} (stream timeout: {}s)",
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
        .add_service(EventQueryServer::new(event_query_proxy))
        .serve(addr)
        .await?;

    Ok(())
}
