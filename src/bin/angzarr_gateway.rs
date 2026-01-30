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
//! ### K8s Mode (production)
//! Auto-discovers services with labels:
//! - `app.kubernetes.io/component=aggregate`
//! - `angzarr.io/domain=<domain-name>`
//!
//! Required for K8s mode:
//! - STREAM_ADDRESS: angzarr-stream service address (e.g., "angzarr-stream:1315")
//!
//! ### Static/Embedded Mode
//! Uses static endpoint configuration:
//! - ANGZARR_STATIC_ENDPOINTS: Comma-separated "domain=address" pairs
//!   e.g., "customer=/tmp/angzarr/aggregate-customer.sock,order=/tmp/angzarr/aggregate-order.sock"
//!
//! Optional:
//! - transport.type: "tcp" or "uds"
//! - transport.tcp.port: Port for TCP (default: 50051)
//! - transport.uds.base_path: Base path for UDS sockets
//! - STREAM_TIMEOUT_SECS: Timeout for event stream (default: 30)
//! - NAMESPACE: Kubernetes namespace for service discovery (default: from downward API)

use std::sync::Arc;
use std::time::Duration;

use backon::Retryable;
use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tracing::{error, info, warn};

use angzarr::config::Config;
use angzarr::discovery::{K8sServiceDiscovery, ServiceDiscovery};
use angzarr::handlers::gateway::{EventQueryProxy, GatewayService};
use angzarr::proto::command_gateway_server::CommandGatewayServer;
use angzarr::proto::event_query_server::EventQueryServer;
use angzarr::proto::event_stream_client::EventStreamClient;
use angzarr::transport::{connect_to_address, grpc_trace_layer, serve_with_transport};
use angzarr::utils::bootstrap::{init_tracing, parse_static_endpoints};
use angzarr::utils::retry::connection_backoff;

const DEFAULT_STREAM_TIMEOUT_SECS: u64 = 30;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    info!("Starting angzarr-gateway service");

    let config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    // Initialize service discovery - try static endpoints first, then K8s
    let discovery: Arc<dyn ServiceDiscovery> =
        if let Ok(endpoints_str) = std::env::var("ANGZARR_STATIC_ENDPOINTS") {
            info!("Using static endpoint configuration");
            let discovery = K8sServiceDiscovery::new_static();

            for (domain, address) in parse_static_endpoints(&endpoints_str) {
                // For UDS addresses, port is 0 (ignored)
                discovery.register_aggregate(&domain, &address, 0).await;
            }

            Arc::new(discovery)
        } else {
            // Fall back to K8s discovery
            match K8sServiceDiscovery::from_env().await {
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
            }
        };

    let domains = discovery.aggregate_domains().await;
    info!(
        domains = ?domains,
        "Service discovery found {} aggregate(s)",
        domains.len()
    );

    // Stream service connection (optional for embedded mode)
    let stream_timeout_secs: u64 = std::env::var("STREAM_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_STREAM_TIMEOUT_SECS);

    let stream_client = match std::env::var("STREAM_ADDRESS") {
        Ok(stream_address) => {
            let stream_addr = stream_address.clone();
            Some(
                (|| {
                    let addr = stream_addr.clone();
                    async move {
                        let channel =
                            connect_to_address(&addr).await.map_err(|e| e.to_string())?;
                        Ok::<_, String>(EventStreamClient::new(channel))
                    }
                })
                .retry(connection_backoff())
                .notify(|err: &String, dur: Duration| {
                    warn!(service = "stream", error = %err, delay = ?dur, "Connection failed, retrying");
                })
                .await?,
            )
        }
        Err(_) => {
            warn!("STREAM_ADDRESS not set - event streaming disabled (embedded mode)");
            None
        }
    };

    let gateway_service = GatewayService::new(
        discovery.clone(),
        stream_client,
        Duration::from_secs(stream_timeout_secs),
    );

    // EventQuery proxy routes queries to appropriate aggregate sidecars by domain
    let event_query_proxy = EventQueryProxy::new(discovery);

    info!(
        stream_timeout = stream_timeout_secs,
        "Angzarr gateway starting"
    );

    // Create health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;

    let router = Server::builder()
        .layer(grpc_trace_layer())
        .add_service(health_service)
        .add_service(CommandGatewayServer::new(gateway_service))
        .add_service(EventQueryServer::new(event_query_proxy));

    serve_with_transport(router, &config.transport, "gateway", None).await?;

    Ok(())
}
