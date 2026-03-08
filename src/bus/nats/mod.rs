//! NATS JetStream EventBus implementation.
//!
//! Provides publish/subscribe for events using NATS JetStream consumers.
//!
//! ## Architecture
//!
//! - **Publishing**: Events published to `{prefix}.events.{domain}.{root}.{edition}`
//! - **Subscribing**: Durable pull consumers filter by domain
//! - **Consumer Groups**: Multiple subscribers with same name share workload
//!
//! # Configuration
//!
//! ```yaml
//! messaging:
//!   type: nats
//!   nats:
//!     url: "nats://localhost:4222"
//!     stream_prefix: "angzarr"
//!     consumer_name: "my-service"
//!     # JetStream-specific
//!     replicas: 3
//!     retention: "limits"  # limits, interest, workqueue
//!     max_age_hours: 168   # 7 days
//! ```

mod bus;
mod config;
mod consumer;

use std::sync::Arc;

use tracing::info;

use super::config::{EventBusMode, MessagingConfig};
use super::error::{BusError, Result};
use super::factory::BusBackend;
use super::traits::EventBus;

pub use bus::NatsEventBus;
pub use config::NatsBusConfig;

// ============================================================================
// Self-Registration
// ============================================================================

inventory::submit! {
    BusBackend {
        try_create: |config, mode| Box::pin(try_create(config, mode)),
    }
}

async fn try_create(
    config: &MessagingConfig,
    mode: EventBusMode,
) -> Option<Result<Arc<dyn EventBus>>> {
    if config.messaging_type != "nats" {
        return None;
    }

    // Connect to NATS
    let client = match async_nats::connect(&config.nats.url).await {
        Ok(c) => c,
        Err(e) => {
            return Some(Err(BusError::Connection(format!(
                "NATS connect failed: {}",
                e
            ))))
        }
    };

    let bus_config = match mode {
        EventBusMode::Publisher => NatsBusConfig {
            prefix: config.nats.stream_prefix.clone(),
            consumer_name: None,
            domain_filter: None,
        },
        EventBusMode::Subscriber { queue, domain } => NatsBusConfig {
            prefix: config.nats.stream_prefix.clone(),
            consumer_name: Some(queue),
            domain_filter: Some(domain),
        },
        EventBusMode::SubscriberAll { queue } => NatsBusConfig {
            prefix: config.nats.stream_prefix.clone(),
            consumer_name: Some(queue),
            domain_filter: None,
        },
    };

    match NatsEventBus::with_config(client, bus_config).await {
        Ok(bus) => {
            info!(messaging_type = "nats", "Event bus initialized");
            Some(Ok(Arc::new(bus)))
        }
        Err(e) => Some(Err(BusError::Connection(format!(
            "NATS setup failed: {}",
            e
        )))),
    }
}
