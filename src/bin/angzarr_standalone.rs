//! angzarr-standalone: All-in-one development server
//!
//! Runs a complete angzarr system in a single process for local development
//! and testing. Not intended for production use.
//!
//! ## Features
//! - In-process event bus (DirectEventBus) - no RabbitMQ required
//! - Direct gRPC connections to projectors and sagas
//! - SQLite or MongoDB storage
//!
//! ## Configuration
//! Set via environment variables or config file (ANGZARR_CONFIG):
//! - STORAGE_TYPE: "sqlite" or "mongodb"
//! - STORAGE_PATH: Database path/URI
//! - COMMAND_HANDLER_PORT: gRPC port for commands (default: 1313)
//! - EVENT_QUERY_PORT: gRPC port for queries (default: 1314)
//!
//! ## Usage
//! ```bash
//! # With defaults
//! angzarr-standalone
//!
//! # With config file
//! ANGZARR_CONFIG=/app/config.yaml angzarr-standalone
//!
//! # With environment overrides
//! STORAGE_TYPE=mongodb STORAGE_PATH=mongodb://localhost:27017 angzarr-standalone
//! ```

use std::sync::Arc;

use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use angzarr::bus::{DirectEventBus, ProjectorConfig, SagaConfig};
use angzarr::clients::{PlaceholderBusinessLogic, StaticBusinessLogicClient};
use angzarr::config::Config;
use angzarr::interfaces::BusinessLogicClient;
use angzarr::proto::{
    business_coordinator_server::BusinessCoordinatorServer, event_query_server::EventQueryServer,
    projector_coordinator_server::ProjectorCoordinatorServer,
    saga_coordinator_server::SagaCoordinatorServer,
};
use angzarr::services::{
    CommandHandlerService, EventQueryService, ProjectorCoordinatorService, SagaCoordinatorService,
};
use angzarr::storage::init_storage;

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

    info!("Starting angzarr-standalone server");

    let (event_store, snapshot_store) = init_storage(&config.storage).await?;
    info!("Storage initialized");

    let event_bus = Arc::new(DirectEventBus::new());

    for projector in &config.projectors {
        if let Err(e) = event_bus
            .add_projector(ProjectorConfig {
                name: projector.name.clone(),
                address: projector.address.clone(),
                synchronous: projector.synchronous,
            })
            .await
        {
            error!("Failed to connect to projector {}: {}", projector.name, e);
        }
    }

    for saga in &config.sagas {
        if let Err(e) = event_bus
            .add_saga(SagaConfig {
                name: saga.name.clone(),
                address: saga.address.clone(),
                synchronous: saga.synchronous,
            })
            .await
        {
            error!("Failed to connect to saga {}: {}", saga.name, e);
        }
    }

    let business_client: Arc<dyn BusinessLogicClient> = if config.business_logic.is_empty() {
        info!("No business logic endpoints configured, using placeholder");
        Arc::new(PlaceholderBusinessLogic::with_defaults())
    } else {
        let addresses = config.business_logic_addresses();
        info!(
            "Connecting to business logic services: {:?}",
            addresses.keys().collect::<Vec<_>>()
        );
        Arc::new(StaticBusinessLogicClient::new(addresses).await?)
    };

    let command_handler = CommandHandlerService::new(
        event_store.clone(),
        snapshot_store.clone(),
        business_client,
        event_bus,
    );

    let event_query = EventQueryService::new(event_store, snapshot_store);
    let projector_coordinator = ProjectorCoordinatorService::new();
    let saga_coordinator = SagaCoordinatorService::new();

    let host = &config.server.host;
    let command_handler_addr =
        format!("{}:{}", host, config.server.command_handler_port).parse()?;
    let event_query_addr = format!("{}:{}", host, config.server.event_query_port).parse()?;

    info!("Command handler listening on {}", command_handler_addr);
    info!("Event query listening on {}", event_query_addr);

    // Create health reporters for each server
    let (mut command_health_reporter, command_health_service) = health_reporter();
    let (mut query_health_reporter, query_health_service) = health_reporter();

    // Set all services as serving
    command_health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;
    query_health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;

    tokio::try_join!(
        Server::builder()
            .add_service(command_health_service)
            .add_service(BusinessCoordinatorServer::new(command_handler))
            .add_service(ProjectorCoordinatorServer::new(projector_coordinator))
            .add_service(SagaCoordinatorServer::new(saga_coordinator))
            .serve(command_handler_addr),
        Server::builder()
            .add_service(query_health_service)
            .add_service(EventQueryServer::new(event_query))
            .serve(event_query_addr),
    )?;

    Ok(())
}
