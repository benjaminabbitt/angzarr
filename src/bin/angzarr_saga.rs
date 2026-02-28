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
//! - ANGZARR_SUBSCRIPTIONS: Event subscriptions (format: "domain:Type1,Type2;domain2")
//! - ANGZARR_STATIC_ENDPOINTS: Static endpoints for multi-domain routing (format: "domain=address,...")
//! - MESSAGING_TYPE: amqp, kafka, or ipc

use std::sync::Arc;
use std::time::Duration;

use backon::Retryable;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use angzarr::bus::{init_event_bus, EventBusMode};
use angzarr::config::{SagaCompensationConfig, STATIC_ENDPOINTS_ENV_VAR};
use angzarr::descriptor::{parse_subscriptions, Target};
use angzarr::handlers::core::saga::SagaEventHandler;
use angzarr::orchestration::saga::grpc::GrpcSagaContextFactory;
use angzarr::proto::saga_service_client::SagaServiceClient;
use angzarr::transport::connect_to_address;
use angzarr::utils::retry::connection_backoff;
use angzarr::utils::sidecar::{bootstrap_sidecar, connect_endpoints, run_subscriber};

/// Environment variable for subscription configuration.
const SUBSCRIPTIONS_ENV_VAR: &str = "ANGZARR_SUBSCRIPTIONS";

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
            Ok::<_, String>(SagaServiceClient::new(channel))
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

    // Get subscriptions from environment variable or config
    let inputs = if let Ok(subs_str) = std::env::var(SUBSCRIPTIONS_ENV_VAR) {
        info!(subscriptions = %subs_str, "Using subscriptions from environment");
        parse_subscriptions(&subs_str)
    } else {
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
        info!(domain = %listen_domain, "Using config-derived subscription");
        vec![Target::domain(listen_domain)]
    };

    info!(
        name = %bootstrap.domain,
        inputs = inputs.len(),
        "Configured saga subscriptions"
    );

    // Build executor, fetcher, and factory from static endpoints
    let endpoints_str = std::env::var(STATIC_ENDPOINTS_ENV_VAR).map_err(|_| {
        error!(
            "{} not set - saga cannot execute commands",
            STATIC_ENDPOINTS_ENV_VAR
        );
        format!("Saga sidecar requires {}", STATIC_ENDPOINTS_ENV_VAR)
    })?;

    info!("Using static endpoint configuration for two-phase saga routing");
    let (executor, fetcher) = connect_endpoints(&endpoints_str).await?;
    let factory = Arc::new(GrpcSagaContextFactory::new(
        Arc::new(Mutex::new(saga_client)),
        publisher,
        SagaCompensationConfig::default(),
        None,
        bootstrap.domain.clone(),
    ));
    let handler = SagaEventHandler::from_factory(factory, executor, Some(fetcher));

    let queue_name = format!("saga-{}", bootstrap.domain);
    run_subscriber(messaging, queue_name, Box::new(handler)).await
}
