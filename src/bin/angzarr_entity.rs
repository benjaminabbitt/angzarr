//! angzarr-entity: Entity sidecar
//!
//! Kubernetes sidecar for business logic services. Handles command processing,
//! event storage, and event publishing.
//!
//! ## Architecture
//! ```text
//! [External Client] -> [angzarr-entity] -> [Business Logic Service]
//!                            |
//!                            v
//!                      [Event Store] + [AMQP Publisher]
//! ```
//!
//! ## Configuration
//! - TARGET_ADDRESS: Business logic gRPC address (e.g., "localhost:50051")
//! - TARGET_DOMAIN: Domain this service handles (e.g., "customers")
//! - STORAGE_TYPE/PATH: Event store configuration
//! - AMQP_URL: Optional RabbitMQ for event publishing

use std::sync::Arc;

use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use angzarr::bus::DirectEventBus;
use angzarr::clients::StaticBusinessLogicClient;
use angzarr::config::Config;
use angzarr::interfaces::BusinessLogicClient;
use angzarr::proto::{
    business_coordinator_server::BusinessCoordinatorServer, event_query_server::EventQueryServer,
};
use angzarr::services::{EntityService, EventQueryService};
use angzarr::storage::init_storage;

use angzarr::bus::AmqpEventBus;
use angzarr::config::MessagingType;
use angzarr::interfaces::EventBus;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_env("ANGZARR_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Starting angzarr-entity sidecar");

    let (event_store, snapshot_store) = init_storage(&config.storage).await?;
    info!("Storage initialized");

    let target = config
        .target
        .as_ref()
        .ok_or("Entity sidecar requires 'target' configuration")?;

    let domain = target
        .domain
        .as_ref()
        .ok_or("Entity sidecar requires 'target.domain' configuration")?;

    info!(
        "Target business logic: {} (domain: {})",
        target.address, domain
    );

    let mut addresses = std::collections::HashMap::new();
    addresses.insert(domain.clone(), format!("http://{}", target.address));
    let business_client: Arc<dyn BusinessLogicClient> =
        Arc::new(StaticBusinessLogicClient::new(addresses).await?);

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
            warn!("No AMQP messaging configured, using direct event bus");
            Arc::new(DirectEventBus::new())
        }
    };

    let entity_service = EntityService::new(
        event_store.clone(),
        snapshot_store.clone(),
        business_client,
        event_bus,
    );

    let event_query = EventQueryService::new(event_store, snapshot_store);

    let host = &config.server.host;
    let addr = format!("{}:{}", host, config.server.entity_port).parse()?;

    info!("Entity sidecar listening on {}", addr);

    // Create health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;

    Server::builder()
        .add_service(health_service)
        .add_service(BusinessCoordinatorServer::new(entity_service))
        .add_service(EventQueryServer::new(event_query))
        .serve(addr)
        .await?;

    Ok(())
}
