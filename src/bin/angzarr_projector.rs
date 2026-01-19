//! angzarr-projector: Projector sidecar
//!
//! Kubernetes sidecar for projector services. Subscribes to events from the
//! message bus (AMQP, Kafka, or IPC) and forwards them to the projector for
//! read model updates.
//!
//! ## Architecture
//! ```text
//! [Event Bus] -> [angzarr-projector] -> [Projector Service]
//!                        |                      |
//!                        v                      v
//!                 [Bus Output] <-------- [Projection]
//!                        |
//!                        v
//!                 [angzarr-stream] -> [Client]
//! ```
//!
//! When STREAM_OUTPUT=true, projector results are published back to the bus
//! as synthetic EventBooks, enabling clients to receive projector output
//! via angzarr-stream.
//!
//! ## Configuration
//! - TARGET_ADDRESS: Projector gRPC address (e.g., "localhost:50051")
//! - TARGET_COMMAND: Optional command to spawn projector (embedded mode)
//! - MESSAGING_TYPE: amqp, kafka, or ipc
//! - STREAM_OUTPUT: Set to "true" to publish projector output (default: false)

use std::path::Path;
use std::time::Duration;

use tracing::{error, info};

use angzarr::bus::{init_event_bus, EventBusMode};
use angzarr::config::Config;
use angzarr::handlers::core::projector::ProjectorEventHandler;
use angzarr::process::{wait_for_ready, ManagedProcess, ProcessEnv};
use angzarr::proto::projector_coordinator_client::ProjectorCoordinatorClient;
use angzarr::transport::connect_to_address;
use angzarr::utils::bootstrap::{connect_with_retry, init_tracing};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Starting angzarr-projector sidecar");

    let target = config
        .target
        .as_ref()
        .ok_or("Projector sidecar requires 'target' configuration")?;

    // Extract projector name for socket naming (use domain if set, otherwise derive from address)
    let projector_name = target
        .domain
        .as_deref()
        .unwrap_or("projector");

    info!("Target projector: {} (name: {})", target.address, projector_name);

    // Spawn projector process if command is configured (embedded mode)
    let _managed_process = if let Some(ref command) = target.command {
        // Extract service_name and domain from target address for socket naming
        // e.g., "/tmp/angzarr/projector-logging-customer.sock" -> service_name="projector-logging", domain="customer"
        let (service_name, domain) = extract_socket_names(&target.address);
        let env = ProcessEnv::from_transport(&config.transport, &service_name, Some(&domain));
        let process = ManagedProcess::spawn(
            command,
            target.working_dir.as_deref(),
            &env,
        )
        .await?;

        // Wait for the service to be ready
        info!("Waiting for projector to be ready...");
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
        .ok_or("Projector sidecar requires 'messaging' configuration")?;

    info!(messaging_type = ?messaging.messaging_type, "Using messaging backend");

    // Check if streaming output is enabled
    let stream_output = std::env::var("STREAM_OUTPUT")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);

    // Connect to projector service with retry
    let projector_addr = target.address.clone();
    let projector_client = connect_with_retry("projector", &target.address, || {
        let addr = projector_addr.clone();
        async move {
            let channel = connect_to_address(&addr).await.map_err(|e| e.to_string())?;
            Ok::<_, String>(ProjectorCoordinatorClient::new(channel))
        }
    })
    .await?;

    // Create publisher if streaming is enabled
    let publisher = if stream_output {
        info!("Streaming output enabled - projector results will be published");
        Some(init_event_bus(messaging, EventBusMode::Publisher).await
            .map_err(|e| -> Box<dyn std::error::Error> { e })?)
    } else {
        info!("Streaming output disabled - projector results will not be published");
        None
    };

    // Create subscriber
    let queue_name = format!("projector-{}", projector_name);
    let subscriber_mode = EventBusMode::SubscriberAll { queue: queue_name };
    let subscriber = init_event_bus(messaging, subscriber_mode).await
        .map_err(|e| -> Box<dyn std::error::Error> { e })?;

    // Create handler with or without streaming capability
    let handler = if let Some(pub_bus) = publisher {
        ProjectorEventHandler::with_publisher(projector_client, pub_bus)
    } else {
        ProjectorEventHandler::new(projector_client)
    };

    subscriber.subscribe(Box::new(handler)).await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    // Start consuming (no-op for AMQP/Kafka, spawns reader for IPC)
    subscriber.start_consuming().await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    info!("Projector sidecar running, press Ctrl+C to exit");

    tokio::signal::ctrl_c().await?;

    Ok(())
}

/// Extract service_name and domain from a UDS socket path.
///
/// For path like "/tmp/angzarr/projector-logging-customer.sock":
/// - Returns ("projector-logging", "customer")
///
/// For path like "/tmp/angzarr/projector-customer.sock":
/// - Returns ("projector", "customer")
fn extract_socket_names(address: &str) -> (String, String) {
    // Get the filename without extension
    let path = Path::new(address);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("projector-unknown");

    // Split on last hyphen to separate domain
    // "projector-logging-customer" -> ("projector-logging", "customer")
    // "projector-customer" -> ("projector", "customer")
    if let Some(last_hyphen) = stem.rfind('-') {
        let service_name = &stem[..last_hyphen];
        let domain = &stem[last_hyphen + 1..];
        (service_name.to_string(), domain.to_string())
    } else {
        // Fallback if no hyphen found
        ("projector".to_string(), stem.to_string())
    }
}
