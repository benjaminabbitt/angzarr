//! angzarr-aggregate: Aggregate sidecar
//!
//! Kubernetes sidecar for business logic services. Handles command processing,
//! event storage, and event publishing.
//!
//! ## Architecture
//! ```text
//! [External Client] -> [angzarr-aggregate] -> [Business Logic Service]
//!                            |
//!                            v
//!                      [Event Store] + [AMQP Publisher]
//!                            |
//!                            v (sync mode)
//!                      [Projector/Saga Coordinators via K8s Labels]
//! ```
//!
//! ## Configuration
//! - TARGET_ADDRESS: Business logic gRPC address (e.g., "localhost:50051")
//! - TARGET_DOMAIN: Domain this service handles (e.g., "customers")
//! - TARGET_COMMAND: Optional command to spawn business logic (embedded mode)
//! - STORAGE_TYPE/PATH: Event store configuration
//! - AMQP_URL: Optional RabbitMQ for event publishing
//!
//! ## Embedded Mode
//! When `target.command` is configured, the sidecar will:
//! 1. Spawn the business logic process with transport configuration
//! 2. Wait for it to become ready (health check)
//! 3. Connect and proceed normally
//!
//! ## Sync Processing (K8s Service Discovery)
//! For synchronous command processing that waits for projectors/sagas:
//! - NAMESPACE or POD_NAMESPACE: K8s namespace for service discovery
//! - Services are discovered by K8s labels:
//!   - app.kubernetes.io/component: projector|saga
//!   - angzarr.io/domain: target domain
//!   - angzarr.io/source-domain: source domain (sagas only)
//! - Service mesh (Linkerd/Istio) handles L7 gRPC load balancing

extern crate std;
use std::sync::Arc;

use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tracing::{error, info, warn};

use angzarr::bus::{AmqpEventBus, EventBus, IpcEventBus, MessagingType, MockEventBus};
use angzarr::config::Config;
use angzarr::discovery::{K8sServiceDiscovery, ServiceDiscovery};
use angzarr::proto::{
    aggregate_client::AggregateClient, aggregate_coordinator_server::AggregateCoordinatorServer,
    event_query_server::EventQueryServer,
};
use angzarr::services::{AggregateService, EventQueryService};
use angzarr::storage::init_storage;
use angzarr::transport::{grpc_trace_layer, serve_with_transport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    angzarr::utils::bootstrap::init_tracing();

    let config_path = angzarr::utils::bootstrap::parse_config_path();
    let config = Config::load(config_path.as_deref()).map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Starting angzarr-aggregate sidecar");

    let (event_store, snapshot_store) = init_storage(&config.storage).await?;
    info!("Storage initialized");

    let target = config
        .target
        .as_ref()
        .ok_or("Aggregate sidecar requires 'target' configuration")?;

    let domain = &target.domain;

    // Resolve address: use explicit if set, otherwise derive from transport
    let address = target.resolve_address(&config.transport, "business");

    info!("Target business logic: {} (domain: {})", address, domain);

    // Connect to business logic
    use angzarr::transport::connect_to_address;
    let channel = connect_to_address(&address).await?;

    let business_client = AggregateClient::new(channel);

    let event_bus: Arc<dyn EventBus> = match &config.messaging {
        Some(messaging) if messaging.messaging_type == MessagingType::Amqp => {
            info!(
                "Connecting to AMQP for event publishing: {}",
                messaging.amqp.url
            );
            let amqp_bus_config = angzarr::bus::AmqpConfig::publisher(&messaging.amqp.url);
            Arc::new(AmqpEventBus::new(amqp_bus_config).await?)
        }
        Some(messaging) if messaging.messaging_type == MessagingType::Ipc => {
            info!(
                "Using IPC for event publishing: {}",
                messaging.ipc.base_path
            );
            let ipc_config = angzarr::bus::IpcConfig::publisher(&messaging.ipc.base_path);
            Arc::new(IpcEventBus::new(ipc_config))
        }
        _ => {
            warn!("No messaging configured, using mock event bus (events not published)");
            Arc::new(MockEventBus::new())
        }
    };

    // Load service discovery for sync processing
    // In standalone mode, ANGZARR_DISCOVERY=static skips K8s entirely
    let discovery: Arc<dyn ServiceDiscovery> =
        if std::env::var("ANGZARR_DISCOVERY").as_deref() == Ok("static") {
            info!("Using static service discovery (standalone mode)");
            Arc::new(K8sServiceDiscovery::new_static())
        } else {
            match K8sServiceDiscovery::from_env().await {
                Ok(discovery) => {
                    let discovery = Arc::new(discovery);
                    if let Err(e) = discovery.initial_sync().await {
                        warn!("Service discovery initial sync failed: {}", e);
                    }
                    discovery.start_watching();
                    info!("Service discovery initialized");
                    discovery
                }
                Err(e) => {
                    warn!(
                        "Service discovery not available (running outside K8s?): {}",
                        e
                    );
                    Arc::new(K8sServiceDiscovery::new_static())
                }
            }
        };

    let aggregate_service = AggregateService::new(
        event_store.clone(),
        snapshot_store.clone(),
        business_client,
        event_bus,
        discovery,
    );

    let event_query = EventQueryService::new(event_store, snapshot_store);

    info!("Aggregate sidecar starting");

    // Create health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;

    let router = Server::builder()
        .layer(grpc_trace_layer())
        .add_service(health_service)
        .add_service(AggregateCoordinatorServer::new(aggregate_service))
        .add_service(EventQueryServer::new(event_query));

    serve_with_transport(router, &config.transport, "aggregate", Some(domain)).await?;

    Ok(())
}
