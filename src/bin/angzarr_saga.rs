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
use angzarr::config::{COMMAND_ADDRESS_ENV_VAR, STATIC_ENDPOINTS_ENV_VAR};
use angzarr::config::SagaCompensationConfig;
use angzarr::handlers::core::saga::SagaEventHandler;
use angzarr::orchestration::command::grpc::SingleClientExecutor;
use angzarr::orchestration::command::CommandExecutor;
use angzarr::orchestration::saga::grpc::GrpcSagaContextFactory;
use angzarr::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use angzarr::proto::saga_client::SagaClient;
use angzarr::proto::GetDescriptorRequest;
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
    let mut saga_client = (|| {
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

    // Fetch descriptor from saga service - includes outputs declared by business logic
    let descriptor = match saga_client.get_descriptor(GetDescriptorRequest {}).await {
        Ok(resp) => resp.into_inner(),
        Err(e) => {
            warn!(error = %e, "Failed to fetch descriptor from saga, using fallback");
            // Fallback: derive input domain from config
            let listen_domain = bootstrap
                .config
                .target
                .as_ref()
                .and_then(|t| t.listen_domain.clone())
                .or_else(|| {
                    bootstrap
                        .config
                        .messaging
                        .as_ref()
                        .and_then(|m| m.amqp.domain.as_ref())
                        .and_then(|d| d.strip_suffix(".*").map(String::from))
                })
                .unwrap_or_else(|| bootstrap.domain.clone());
            angzarr::proto::ComponentDescriptor {
                name: bootstrap.domain.clone(),
                component_type: "saga".to_string(),
                inputs: vec![angzarr::proto::Target {
                    domain: listen_domain,
                    types: vec![],
                }],
                outputs: vec![],
            }
        }
    };
    info!(
        name = %descriptor.name,
        inputs = descriptor.inputs.len(),
        outputs = descriptor.outputs.len(),
        "Fetched saga descriptor"
    );
    // Write descriptor to pod annotation for K8s-native topology discovery
    if let Err(e) = angzarr::discovery::k8s::write_descriptor_if_k8s(&descriptor).await {
        warn!(error = %e, "Failed to write descriptor annotation");
    }

    // Build executor, fetcher, and factory based on configuration mode
    let handler = if let Ok(endpoints_str) = std::env::var(STATIC_ENDPOINTS_ENV_VAR) {
        info!("Using static endpoint configuration for two-phase saga routing");
        let (executor, fetcher) = connect_endpoints(&endpoints_str).await?;
        let factory = Arc::new(GrpcSagaContextFactory::new(
            Arc::new(Mutex::new(saga_client)),
            publisher,
            SagaCompensationConfig::default(),
            None,
            bootstrap.domain.clone(),
        ));
        SagaEventHandler::from_factory(factory, executor, Some(fetcher))
    } else if let Ok(command_address) = std::env::var(COMMAND_ADDRESS_ENV_VAR) {
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
            bootstrap.domain.clone(),
        ));
        SagaEventHandler::from_factory(factory, executor, None)
    } else {
        error!("Neither {} nor {} set - saga cannot execute commands", STATIC_ENDPOINTS_ENV_VAR, COMMAND_ADDRESS_ENV_VAR);
        return Err(format!("Saga sidecar requires {} or {}", STATIC_ENDPOINTS_ENV_VAR, COMMAND_ADDRESS_ENV_VAR).into());
    };

    let queue_name = format!("saga-{}", bootstrap.domain);
    run_subscriber(messaging, queue_name, Box::new(handler)).await
}
