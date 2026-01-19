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

use std::sync::Arc;
use std::time::Duration;

use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tracing::{error, info, warn};

use angzarr::bus::{AmqpEventBus, EventBus, MessagingType, MockEventBus};
use angzarr::config::Config;
use angzarr::discovery::ServiceDiscovery;
use angzarr::process::{wait_for_ready, ManagedProcess, ProcessEnv};
use angzarr::proto::{
    aggregate_client::AggregateClient, aggregate_coordinator_server::AggregateCoordinatorServer,
    event_query_server::EventQueryServer,
};
use angzarr::services::{AggregateService, EventQueryService};
use angzarr::storage::init_storage;
use angzarr::transport::serve_with_transport;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    angzarr::utils::bootstrap::init_tracing();

    let config = Config::load().map_err(|e| {
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

    let domain = target
        .domain
        .as_ref()
        .ok_or("Aggregate sidecar requires 'target.domain' configuration")?;

    info!(
        "Target business logic: {} (domain: {})",
        target.address, domain
    );

    // Spawn business logic process if command is configured (embedded mode)
    let _managed_process = if let Some(ref command) = target.command {
        let env = ProcessEnv::from_transport(&config.transport, "business", Some(domain));
        let process = ManagedProcess::spawn(
            command,
            target.working_dir.as_deref(),
            &env,
        )
        .await?;

        // Wait for the service to be ready
        info!("Waiting for business logic to be ready...");
        let channel = wait_for_ready(
            &target.address,
            Duration::from_secs(30),
            Duration::from_millis(500),
        )
        .await?;

        // Return both the process (to keep it alive) and the channel
        Some((process, channel))
    } else {
        None
    };

    // Connect to business logic (or use the channel from wait_for_ready)
    let channel = if let Some((_, ref channel)) = _managed_process {
        channel.clone()
    } else {
        use angzarr::transport::connect_to_address;
        connect_to_address(&target.address).await?
    };

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
        _ => {
            warn!("No AMQP messaging configured, using mock event bus (events not published)");
            Arc::new(MockEventBus::new())
        }
    };

    // Load service discovery for sync processing (K8s label-based discovery)
    let discovery = match ServiceDiscovery::from_env().await {
        Ok(discovery) => {
            let discovery = Arc::new(discovery);
            // Perform initial sync and start watching
            if let Err(e) = discovery.initial_sync().await {
                warn!("Service discovery initial sync failed: {}", e);
            }
            discovery.start_watching();
            info!("Service discovery initialized");
            Some(discovery)
        }
        Err(e) => {
            warn!("Service discovery not available (running outside K8s?): {}", e);
            None
        }
    };

    let mut aggregate_service = AggregateService::new(
        event_store.clone(),
        snapshot_store.clone(),
        business_client,
        event_bus,
    );

    // Wire up service discovery for sync processing
    if let Some(discovery) = discovery {
        aggregate_service = aggregate_service.with_discovery(discovery);
    }

    let event_query = EventQueryService::new(event_store, snapshot_store);

    info!("Aggregate sidecar starting");

    // Create health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;

    let router = Server::builder()
        .add_service(health_service)
        .add_service(AggregateCoordinatorServer::new(aggregate_service))
        .add_service(EventQueryServer::new(event_query));

    serve_with_transport(router, &config.transport, "aggregate", Some(domain)).await?;

    Ok(())
}
