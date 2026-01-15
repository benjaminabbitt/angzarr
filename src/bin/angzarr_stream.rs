//! angzarr-stream: Event streaming service
//!
//! Central infrastructure service that streams events to registered subscribers.
//! Receives events from AMQP and forwards to subscribers filtered by correlation ID.
//!
//! ## Architecture
//! ```text
//! [AMQP Events] -> [angzarr-stream] <- [gRPC Subscribe]
//!                        |                    |
//!                        v                    v
//!                  (correlation ID match) -> [angzarr-gateway]
//! ```
//!
//! ## Configuration
//! - AMQP_URL: RabbitMQ connection string
//! - GRPC_PORT: Port for EventStream gRPC service (default: 1315)

use std::net::SocketAddr;

use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use angzarr::bus::{AmqpConfig, AmqpEventBus};
use angzarr::config::{Config, MessagingType};
use angzarr::handlers::stream::{StreamEventHandler, StreamService};
use angzarr::interfaces::EventBus;
use angzarr::proto::event_stream_server::EventStreamServer;

const DEFAULT_GRPC_PORT: u16 = 1315;

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

    info!("Starting angzarr-stream service");

    let messaging = config
        .messaging
        .as_ref()
        .filter(|m| m.messaging_type == MessagingType::Amqp)
        .ok_or("angzarr-stream requires 'messaging.type: amqp' configuration")?;

    let amqp_config = &messaging.amqp;

    info!("Subscribing to all AMQP events");

    // Create stream service
    let stream_service = StreamService::new();

    // Create event handler that forwards to stream subscribers
    let handler = StreamEventHandler::new(&stream_service);

    // Subscribe to all events from AMQP
    let queue_name = format!("stream-{}", std::process::id());
    let amqp_bus_config = AmqpConfig::subscriber_all(&amqp_config.url, &queue_name);
    let amqp_bus = AmqpEventBus::new(amqp_bus_config).await?;

    amqp_bus.subscribe(Box::new(handler)).await?;
    amqp_bus.start_consuming().await?;

    // Start gRPC server
    let grpc_port = std::env::var("GRPC_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_GRPC_PORT);

    let addr: SocketAddr = format!("0.0.0.0:{}", grpc_port).parse()?;

    info!("EventStream gRPC server listening on {}", addr);

    // Create health reporter
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;

    Server::builder()
        .add_service(health_service)
        .add_service(EventStreamServer::new(stream_service))
        .serve(addr)
        .await?;

    Ok(())
}
