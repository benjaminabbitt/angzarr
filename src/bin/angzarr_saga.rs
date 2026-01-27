//! angzarr-saga: Saga sidecar
//!
//! Kubernetes sidecar for saga services. Subscribes to events from the
//! message bus (AMQP, Kafka, or IPC), forwards to saga for processing,
//! and executes resulting commands via the command handler.
//!
//! ## Two-Phase Saga Protocol
//! 1. **Prepare**: Saga declares which destination aggregates it needs to read
//! 2. **Fetch**: Sidecar fetches destination EventBooks via EventQuery
//! 3. **Execute**: Saga receives source + destinations, produces commands
//!
//! ## Architecture
//! ```text
//! [Event Bus] -> [angzarr-saga] -> [Saga.Prepare] -> destinations
//!                        |                               |
//!                        v                               v
//!              [EventQuery.GetEventBook] <-------- fetch state
//!                        |
//!                        v
//!              [Saga.Execute(source, destinations)] -> commands
//!                        |
//!                        v
//!              [AggregateCoordinator.Handle] -> events
//!                        |
//!                        v
//!                  [Event Bus] -> [Client]
//! ```
//!
//! ## Configuration
//! - TARGET_ADDRESS: Saga gRPC address (e.g., "localhost:50051")
//! - TARGET_COMMAND: Optional command to spawn saga (embedded mode)
//! - ANGZARR_STATIC_ENDPOINTS: Static endpoints for multi-domain routing (format: "domain=address,...")
//!   Enables two-phase protocol with EventQuery support
//! - COMMAND_ADDRESS: Single command handler (fallback, no two-phase support)
//! - MESSAGING_TYPE: amqp, kafka, or ipc

use std::collections::HashMap;

use std::time::Duration;

use tracing::{error, info, warn};

use angzarr::bus::{init_event_bus, EventBusMode};
use angzarr::config::Config;
use angzarr::handlers::core::saga::{CommandRouter, EventQueryRouter, SagaEventHandler};
use angzarr::process::{wait_for_ready, ManagedProcess, ProcessEnv};
use angzarr::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use angzarr::proto::event_query_client::EventQueryClient;
use angzarr::proto::saga_client::SagaClient;
use angzarr::transport::connect_to_address;
use angzarr::utils::bootstrap::{connect_with_retry, init_tracing};

/// Parse static endpoints from environment variable.
///
/// Format: "domain=address,domain=address,..."
/// Example: "customer=/tmp/angzarr/aggregate-customer.sock,order=/tmp/angzarr/aggregate-order.sock"
fn parse_static_endpoints(endpoints_str: &str) -> Vec<(String, String)> {
    endpoints_str
        .split(',')
        .filter_map(|pair| {
            let parts: Vec<&str> = pair.trim().splitn(2, '=').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

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
    let saga_name = &target.domain;

    // Resolve address: use explicit if set, otherwise derive from transport
    let address = target.resolve_address(&config.transport, "saga");

    info!("Target saga: {} (name: {})", address, saga_name);

    // Get command: prefer env var (for standalone mode), fall back to config
    let command = match std::env::var("ANGZARR__TARGET__COMMAND_JSON") {
        Ok(json) => serde_json::from_str::<Vec<String>>(&json).unwrap_or_else(|_| {
            warn!("Failed to parse ANGZARR__TARGET__COMMAND_JSON, falling back to config");
            target.command.clone()
        }),
        Err(_) => target.command.clone(),
    };

    // Spawn saga process if command is configured (embedded mode)
    let _managed_process = if !command.is_empty() {
        let env = ProcessEnv::from_transport(&config.transport, "saga", Some(saga_name));
        let process =
            ManagedProcess::spawn(&command, target.working_dir.as_deref(), &env, None).await?;

        // Wait for the service to be ready
        info!("Waiting for saga to be ready...");
        wait_for_ready(
            &address,
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
    let saga_addr = address.clone();
    let saga_client = connect_with_retry("saga", &address, || {
        let addr = saga_addr.clone();
        async move {
            let channel = connect_to_address(&addr).await.map_err(|e| e.to_string())?;
            Ok::<_, String>(SagaClient::new(channel))
        }
    })
    .await?;

    // Create publisher for saga-produced event books
    let publisher = init_event_bus(messaging, EventBusMode::Publisher)
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e })?;

    // Create subscriber
    let queue_name = format!("saga-{}", saga_name);
    let subscriber_mode = EventBusMode::SubscriberAll { queue: queue_name };
    let subscriber = init_event_bus(messaging, subscriber_mode)
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e })?;

    // Try static endpoints first (multi-domain routing), then COMMAND_ADDRESS (single domain)
    let handler = if let Ok(endpoints_str) = std::env::var("ANGZARR_STATIC_ENDPOINTS") {
        info!("Using static endpoint configuration for two-phase saga routing");
        let endpoints = parse_static_endpoints(&endpoints_str);

        // Connect to all aggregates - both for commands and event queries
        // (EventQuery runs on the same aggregate sidecar)
        let mut command_clients = HashMap::new();
        let mut query_clients = HashMap::new();

        for (domain, address) in endpoints {
            // Connect AggregateCoordinator client for commands
            let addr = address.clone();
            let cmd_client =
                connect_with_retry(&format!("aggregate-{}", domain), &address, || {
                    let a = addr.clone();
                    async move {
                        let channel = connect_to_address(&a).await.map_err(|e| e.to_string())?;
                        Ok::<_, String>(AggregateCoordinatorClient::new(channel))
                    }
                })
                .await?;
            command_clients.insert(domain.clone(), cmd_client);

            // Connect EventQuery client for fetching destination state
            let addr = address.clone();
            let query_client =
                connect_with_retry(&format!("event-query-{}", domain), &address, || {
                    let a = addr.clone();
                    async move {
                        let channel = connect_to_address(&a).await.map_err(|e| e.to_string())?;
                        Ok::<_, String>(EventQueryClient::new(channel))
                    }
                })
                .await?;
            query_clients.insert(domain.clone(), query_client);

            info!(domain = %domain, address = %address, "Connected to aggregate (commands + queries)");
        }

        let command_router = CommandRouter::new(command_clients);
        let event_query_router = EventQueryRouter::new(query_clients);
        SagaEventHandler::with_routers(saga_client, command_router, event_query_router, publisher)
    } else if let Ok(command_address) = std::env::var("COMMAND_ADDRESS") {
        // Fall back to single command handler (no two-phase support)
        let cmd_addr = command_address.clone();
        let client = connect_with_retry("command handler", &command_address, || {
            let addr = cmd_addr.clone();
            async move {
                let channel = connect_to_address(&addr).await.map_err(|e| e.to_string())?;
                Ok::<_, String>(AggregateCoordinatorClient::new(channel))
            }
        })
        .await?;
        warn!("Using single COMMAND_ADDRESS - two-phase saga protocol not supported");
        SagaEventHandler::with_command_handler(saga_client, client, publisher)
    } else {
        warn!("Neither ANGZARR_STATIC_ENDPOINTS nor COMMAND_ADDRESS set - saga-produced commands will not be executed");
        SagaEventHandler::new(saga_client, publisher)
    };

    subscriber
        .subscribe(Box::new(handler))
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    // Start consuming (no-op for AMQP/Kafka, spawns reader for IPC)
    subscriber
        .start_consuming()
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    info!("Saga sidecar running, press Ctrl+C to exit");

    tokio::signal::ctrl_c().await?;

    Ok(())
}
