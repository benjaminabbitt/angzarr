//! angzarr-projector: Projector sidecar
//!
//! Kubernetes sidecar for projector services. Subscribes to events from AMQP
//! and forwards them to the projector for read model updates.
//!
//! ## Architecture
//! ```text
//! [AMQP Events] -> [angzarr-projector] -> [Projector Service]
//!                        |                      |
//!                        v                      v
//!                 [AMQP Output] <-------- [Projection]
//!                        |
//!                        v
//!                 [angzarr-stream] -> [Client]
//! ```
//!
//! When STREAM_OUTPUT=true, projector results are published back to AMQP
//! as synthetic EventBooks, enabling clients to receive projector output
//! via angzarr-stream.
//!
//! ## Configuration
//! - TARGET_ADDRESS: Projector gRPC address (e.g., "localhost:50051")
//! - AMQP_URL: RabbitMQ connection string
//! - AMQP_DOMAIN: Domain to subscribe to (or "#" for all)
//! - STREAM_OUTPUT: Set to "true" to publish projector output to AMQP (default: false)

use tracing::{error, info};

use angzarr::utils::bootstrap::{connect_with_retry, init_tracing};

use angzarr::bus::{AmqpConfig, AmqpEventBus, EventBus, MessagingType};
use angzarr::config::Config;
use angzarr::handlers::projector::ProjectorEventHandler;
use angzarr::proto::projector_coordinator_client::ProjectorCoordinatorClient;

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

    info!("Target projector: {}", target.address);

    let messaging = config
        .messaging
        .as_ref()
        .filter(|m| m.messaging_type == MessagingType::Amqp)
        .ok_or("Projector sidecar requires 'messaging.type: amqp' configuration")?;

    let amqp_config = &messaging.amqp;

    let domains = amqp_config
        .domains
        .clone()
        .or_else(|| amqp_config.domain.as_ref().map(|d| vec![d.clone()]))
        .unwrap_or_else(|| vec!["#".to_string()]);

    info!("Subscribing to AMQP events for domains: {:?}", domains);

    // Check if streaming output is enabled
    let stream_output = std::env::var("STREAM_OUTPUT")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);

    // Connect to projector service with retry
    let projector_address = format!("http://{}", target.address);
    let projector_client = connect_with_retry("projector", &target.address, || {
        ProjectorCoordinatorClient::connect(projector_address.clone())
    })
    .await?;

    // Create AMQP publisher if streaming is enabled
    let publisher = if stream_output {
        info!("Streaming output enabled - projector results will be published to AMQP");
        let publisher_config = AmqpConfig::publisher(&amqp_config.url);
        Some(AmqpEventBus::new(publisher_config).await?)
    } else {
        info!("Streaming output disabled - projector results will not be published");
        None
    };

    // Create AMQP subscriber
    let queue_name = format!("projector-{}", std::process::id());
    let subscriber_config = if domains.len() == 1 && domains[0] != "#" {
        AmqpConfig::subscriber(&amqp_config.url, &queue_name, &domains[0])
    } else {
        AmqpConfig::subscriber_all(&amqp_config.url, &queue_name)
    };
    let subscriber = AmqpEventBus::new(subscriber_config).await?;

    // Create handler with or without streaming capability
    let handler = if let Some(pub_bus) = publisher {
        ProjectorEventHandler::with_publisher(projector_client, pub_bus)
    } else {
        ProjectorEventHandler::new(projector_client)
    };

    subscriber.subscribe(Box::new(handler)).await?;
    subscriber.start_consuming().await?;

    info!("Projector sidecar running, press Ctrl+C to exit");

    tokio::signal::ctrl_c().await?;

    Ok(())
}
