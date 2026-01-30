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

use std::sync::Arc;
use std::time::Duration;

use backon::Retryable;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use angzarr::bus::{init_event_bus, EventBusMode};
use angzarr::orchestration::aggregate::DEFAULT_EDITION;
use angzarr::clients::SagaCompensationConfig;
use angzarr::handlers::core::saga::SagaEventHandler;
use angzarr::orchestration::command::grpc::SingleClientExecutor;
use angzarr::orchestration::command::CommandExecutor;
use angzarr::orchestration::saga::grpc::GrpcSagaContextFactory;
use angzarr::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use angzarr::proto::saga_client::SagaClient;
use angzarr::transport::connect_to_address;
use angzarr::utils::retry::connection_backoff;
use angzarr::utils::sidecar::{bootstrap_sidecar, connect_endpoints, run_subscriber};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap = bootstrap_sidecar("saga").await?;

    let messaging = bootstrap
        .config
        .messaging
        .as_ref()
        .ok_or("Saga sidecar requires 'messaging' configuration")?;

    info!(messaging_type = ?messaging.messaging_type, "Using messaging backend");

    // Connect to saga service with retry
    let saga_addr = bootstrap.address.clone();
    let saga_client = (|| {
        let addr = saga_addr.clone();
        async move {
            let channel = connect_to_address(&addr).await.map_err(|e| e.to_string())?;
            Ok::<_, String>(SagaClient::new(channel))
        }
    })
    .retry(connection_backoff())
    .notify(|err: &String, dur: Duration| {
        warn!(service = "saga", error = %err, delay = ?dur, "Connection failed, retrying");
    })
    .await?;

    // Create publisher for saga-produced event books
    let publisher = init_event_bus(messaging, EventBusMode::Publisher)
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e })?;

    // Build executor, fetcher, and factory based on configuration mode
    let handler = if let Ok(endpoints_str) = std::env::var("ANGZARR_STATIC_ENDPOINTS") {
        info!("Using static endpoint configuration for two-phase saga routing");
        let (executor, fetcher) = connect_endpoints(&endpoints_str).await?;
        let factory = Arc::new(GrpcSagaContextFactory::new(
            Arc::new(Mutex::new(saga_client)),
            publisher,
            SagaCompensationConfig::default(),
            None,
            format!("{DEFAULT_EDITION}.{}", bootstrap.domain),
        ));
        SagaEventHandler::from_factory(factory, executor, Some(fetcher))
    } else if let Ok(command_address) = std::env::var("COMMAND_ADDRESS") {
        let cmd_addr = command_address.clone();
        let client = (|| {
            let addr = cmd_addr.clone();
            async move {
                let channel = connect_to_address(&addr).await.map_err(|e| e.to_string())?;
                Ok::<_, String>(AggregateCoordinatorClient::new(channel))
            }
        })
        .retry(connection_backoff())
        .notify(|err: &String, dur: Duration| {
            warn!(service = "command handler", error = %err, delay = ?dur, "Connection failed, retrying");
        })
        .await?;
        warn!("Using single COMMAND_ADDRESS - two-phase saga protocol not supported");
        let comp_handler = Arc::new(Mutex::new(client));
        let executor: Arc<dyn CommandExecutor> =
            Arc::new(SingleClientExecutor::new(comp_handler.clone()));
        let factory = Arc::new(GrpcSagaContextFactory::new(
            Arc::new(Mutex::new(saga_client)),
            publisher,
            SagaCompensationConfig::default(),
            Some(comp_handler),
            format!("{DEFAULT_EDITION}.{}", bootstrap.domain),
        ));
        SagaEventHandler::from_factory(factory, executor, None)
    } else {
        error!("Neither ANGZARR_STATIC_ENDPOINTS nor COMMAND_ADDRESS set - saga cannot execute commands");
        return Err("Saga sidecar requires ANGZARR_STATIC_ENDPOINTS or COMMAND_ADDRESS".into());
    };

    let queue_name = format!("saga-{}", bootstrap.domain);
    run_subscriber(messaging, queue_name, Box::new(handler)).await
}
