//! angzarr-saga: Saga sidecar
//!
//! Kubernetes sidecar for saga services. Subscribes to events from AMQP,
//! forwards to saga for processing, and executes resulting commands via
//! the command handler.
//!
//! ## Architecture
//! ```text
//! [AMQP Events] -> [angzarr-saga] -> [Saga Service]
//!                        |                 |
//!                        v                 v
//!              [angzarr-entity] <-- [Commands]
//!                        |
//!                        v
//!                  [AMQP Events] -> [angzarr-stream] -> [Client]
//! ```
//!
//! ## Configuration
//! - TARGET_ADDRESS: Saga gRPC address (e.g., "localhost:50051")
//! - COMMAND_ADDRESS: Command handler address for executing saga commands (e.g., "angzarr-entity:1313")
//! - AMQP_URL: RabbitMQ connection string
//! - AMQP_DOMAIN: Domain to subscribe to (or "#" for all)

use tracing::{error, info, warn};

use angzarr::utils::bootstrap::{connect_with_retry, init_tracing};

use angzarr::bus::{AmqpConfig, AmqpEventBus, EventBus, MessagingType};
use angzarr::config::Config;
use angzarr::handlers::core::saga::SagaEventHandler;
use angzarr::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use angzarr::proto::saga_coordinator_client::SagaCoordinatorClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Starting angzarr-saga sidecar");

    let target = config
        .target
        .as_ref()
        .ok_or("Saga sidecar requires 'target' configuration")?;

    info!("Target saga: {}", target.address);

    let messaging = config
        .messaging
        .as_ref()
        .filter(|m| m.messaging_type == MessagingType::Amqp)
        .ok_or("Saga sidecar requires 'messaging.type: amqp' configuration")?;

    let amqp_config = &messaging.amqp;

    let domains = amqp_config
        .domains
        .clone()
        .or_else(|| amqp_config.domain.as_ref().map(|d| vec![d.clone()]))
        .unwrap_or_else(|| vec!["#".to_string()]);

    info!("Subscribing to AMQP events for domains: {:?}", domains);

    // Connect to saga service with retry
    let saga_address = format!("http://{}", target.address);
    let saga_client = connect_with_retry("saga", &target.address, || {
        SagaCoordinatorClient::connect(saga_address.clone())
    })
    .await?;

    // Connect to command handler if configured (for executing saga-produced commands)
    let command_handler = if let Ok(command_address) = std::env::var("COMMAND_ADDRESS") {
        let cmd_url = format!("http://{}", command_address);
        let client = connect_with_retry("command handler", &command_address, || {
            AggregateCoordinatorClient::connect(cmd_url.clone())
        })
        .await?;
        Some(client)
    } else {
        warn!("COMMAND_ADDRESS not set - saga-produced commands will not be executed");
        None
    };

    // Create AMQP publisher for saga-produced event books
    let publisher_config = AmqpConfig::publisher(&amqp_config.url);
    let publisher = AmqpEventBus::new(publisher_config).await?;

    // Create AMQP subscriber
    let queue_name = format!("saga-{}", std::process::id());
    let subscriber_config = if domains.len() == 1 && domains[0] != "#" {
        AmqpConfig::subscriber(&amqp_config.url, &queue_name, &domains[0])
    } else {
        AmqpConfig::subscriber_all(&amqp_config.url, &queue_name)
    };
    let subscriber = AmqpEventBus::new(subscriber_config).await?;

    // Create handler with or without command execution capability
    let handler = if let Some(cmd_handler) = command_handler {
        SagaEventHandler::with_command_handler(saga_client, cmd_handler, publisher)
    } else {
        SagaEventHandler::new(saga_client, publisher)
    };

    subscriber.subscribe(Box::new(handler)).await?;
    subscriber.start_consuming().await?;

    info!("Saga sidecar running, press Ctrl+C to exit");

    tokio::signal::ctrl_c().await?;

    Ok(())
}
