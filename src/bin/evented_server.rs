//! Evented server binary.

use std::sync::Arc;

use tonic::transport::Server;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use evented::bus::{DirectEventBus, ProjectorConfig, SagaConfig};
use evented::clients::{PlaceholderBusinessLogic, StaticBusinessLogicClient};
use evented::config::Config;
use evented::interfaces::BusinessLogicClient;
use evented::proto::{
    business_coordinator_server::BusinessCoordinatorServer, event_query_server::EventQueryServer,
    projector_coordinator_server::ProjectorCoordinatorServer,
    saga_coordinator_server::SagaCoordinatorServer,
};
use evented::services::{
    CommandHandlerService, EventQueryService, ProjectorCoordinatorService, SagaCoordinatorService,
};
use evented::storage::{SqliteEventStore, SqliteSnapshotStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Starting evented server");
    info!(
        "Storage: {} at {}",
        config.storage.storage_type, config.storage.path
    );

    // Ensure data directory exists
    if let Some(parent) = std::path::Path::new(&config.storage.path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Initialize storage
    let pool =
        sqlx::SqlitePool::connect(&format!("sqlite:{}?mode=rwc", config.storage.path)).await?;

    let event_store = Arc::new(SqliteEventStore::new(pool.clone()));
    event_store.init().await?;

    let snapshot_store = Arc::new(SqliteSnapshotStore::new(pool));
    snapshot_store.init().await?;

    info!("Storage initialized");

    // Initialize event bus
    let event_bus = Arc::new(DirectEventBus::new());

    // Add projector endpoints
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

    // Add saga endpoints
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

    // Initialize business logic client
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

    // Create services
    let command_handler = CommandHandlerService::new(
        event_store.clone(),
        snapshot_store.clone(),
        business_client,
        event_bus,
    );

    let event_query = EventQueryService::new(event_store, snapshot_store);
    let projector_coordinator = ProjectorCoordinatorService::new();
    let saga_coordinator = SagaCoordinatorService::new();

    // Start servers
    let host = &config.server.host;
    let command_handler_addr =
        format!("{}:{}", host, config.server.command_handler_port).parse()?;
    let event_query_addr = format!("{}:{}", host, config.server.event_query_port).parse()?;

    info!("Command handler listening on {}", command_handler_addr);
    info!("Event query listening on {}", event_query_addr);

    // Run both servers concurrently
    tokio::try_join!(
        Server::builder()
            .add_service(BusinessCoordinatorServer::new(command_handler))
            .add_service(ProjectorCoordinatorServer::new(projector_coordinator))
            .add_service(SagaCoordinatorServer::new(saga_coordinator))
            .serve(command_handler_addr),
        Server::builder()
            .add_service(EventQueryServer::new(event_query))
            .serve(event_query_addr),
    )?;

    Ok(())
}
