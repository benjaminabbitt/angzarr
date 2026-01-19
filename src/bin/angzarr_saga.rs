//! angzarr-saga: Saga sidecar
//!
//! Kubernetes sidecar for saga services. Subscribes to events from the
//! message bus (AMQP, Kafka, or IPC), forwards to saga for processing,
//! and executes resulting commands via the command handler.
//!
//! ## Architecture
//! ```text
//! [Event Bus] -> [angzarr-saga] -> [Saga Service]
//!                        |                 |
//!                        v                 v
//!              [angzarr-entity] <-- [Commands]
//!                        |
//!                        v
//!                  [Event Bus] -> [angzarr-stream] -> [Client]
//! ```
//!
//! ## Configuration
//! - TARGET_ADDRESS: Saga gRPC address (e.g., "localhost:50051")
//! - TARGET_COMMAND: Optional command to spawn saga (embedded mode)
//! - COMMAND_ADDRESS: Command handler address for executing saga commands
//! - MESSAGING_TYPE: amqp, kafka, or ipc

use std::time::Duration;

use tracing::{error, info, warn};

use angzarr::bus::{init_event_bus, EventBusMode};
use angzarr::config::Config;
use angzarr::handlers::core::saga::SagaEventHandler;
use angzarr::process::{wait_for_ready, ManagedProcess, ProcessEnv};
use angzarr::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use angzarr::proto::saga_coordinator_client::SagaCoordinatorClient;
use angzarr::transport::connect_to_address;
use angzarr::utils::bootstrap::{connect_with_retry, init_tracing};

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

    // Extract saga name for socket naming
    let saga_name = target
        .domain
        .as_deref()
        .unwrap_or("saga");

    info!("Target saga: {} (name: {})", target.address, saga_name);

    // Spawn saga process if command is configured (embedded mode)
    let _managed_process = if let Some(ref command) = target.command {
        let env = ProcessEnv::from_transport(&config.transport, "saga", Some(saga_name));
        let process = ManagedProcess::spawn(
            command,
            target.working_dir.as_deref(),
            &env,
        )
        .await?;

        // Wait for the service to be ready
        info!("Waiting for saga to be ready...");
        wait_for_ready(
            &target.address,
            Duration::from_secs(30),
            Duration::from_millis(500),
        )
        .await?;

        Some(process)
    } else {
        None
    };

    let messaging = config
        .messaging
        .as_ref()
        .ok_or("Saga sidecar requires 'messaging' configuration")?;

    info!(messaging_type = ?messaging.messaging_type, "Using messaging backend");

    // Connect to saga service with retry
    let saga_addr = target.address.clone();
    let saga_client = connect_with_retry("saga", &target.address, || {
        let addr = saga_addr.clone();
        async move {
            let channel = connect_to_address(&addr).await.map_err(|e| e.to_string())?;
            Ok::<_, String>(SagaCoordinatorClient::new(channel))
        }
    })
    .await?;

    // Connect to command handler if configured (for executing saga-produced commands)
    let command_handler = if let Ok(command_address) = std::env::var("COMMAND_ADDRESS") {
        let cmd_addr = command_address.clone();
        let client = connect_with_retry("command handler", &command_address, || {
            let addr = cmd_addr.clone();
            async move {
                let channel = connect_to_address(&addr).await.map_err(|e| e.to_string())?;
                Ok::<_, String>(AggregateCoordinatorClient::new(channel))
            }
        })
        .await?;
        Some(client)
    } else {
        warn!("COMMAND_ADDRESS not set - saga-produced commands will not be executed");
        None
    };

    // Create publisher for saga-produced event books
    let publisher = init_event_bus(messaging, EventBusMode::Publisher).await
        .map_err(|e| -> Box<dyn std::error::Error> { e })?;

    // Create subscriber
    let queue_name = format!("saga-{}", saga_name);
    let subscriber_mode = EventBusMode::SubscriberAll { queue: queue_name };
    let subscriber = init_event_bus(messaging, subscriber_mode).await
        .map_err(|e| -> Box<dyn std::error::Error> { e })?;

    // Create handler with or without command execution capability
    let handler = if let Some(cmd_handler) = command_handler {
        SagaEventHandler::with_command_handler(saga_client, cmd_handler, publisher)
    } else {
        SagaEventHandler::new(saga_client, publisher)
    };

    subscriber.subscribe(Box::new(handler)).await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    // Start consuming (no-op for AMQP/Kafka, spawns reader for IPC)
    subscriber.start_consuming().await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    info!("Saga sidecar running, press Ctrl+C to exit");

    tokio::signal::ctrl_c().await?;

    Ok(())
}
