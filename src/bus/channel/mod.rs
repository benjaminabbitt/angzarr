//! In-memory channel-based event bus for standalone mode.
//!
//! Uses tokio broadcast channels for pub/sub within a single process.
//! Ideal for local development and testing without external dependencies.
//!
//! ## Trace Context Propagation
//!
//! Channel bus does **not** implement explicit trace context propagation.
//! This is intentional:
//!
//! 1. **Same process**: All publishers and subscribers run in the same tokio
//!    runtime. The tracing context flows naturally through the async task
//!    hierarchy without explicit injection/extraction.
//!
//! 2. **No serialization boundary**: Unlike distributed buses, channel messages
//!    are `Arc<EventBook>` passed by reference—no wire protocol to carry headers.
//!
//! 3. **Testing focus**: Channel bus is for unit/integration tests where
//!    distributed tracing is not a concern.
//!
//! For production distributed tracing, use AMQP, Kafka, or SNS/SQS buses which
//! implement full W3C TraceContext propagation via [`crate::utils::tracing`].

mod command_bus;
mod config;
mod event_bus;

use std::sync::Arc;

use tracing::info;

use super::config::EventBusMode;
use super::factory::BusBackend;
use super::traits::EventBus;

pub use command_bus::ChannelCommandBus;
pub use config::{domain_matches, ChannelConfig};
pub use event_bus::ChannelEventBus;

// ============================================================================
// Self-Registration
// ============================================================================

inventory::submit! {
    BusBackend {
        try_create: |config, mode| {
            // Clone what we need before creating the 'static future
            let messaging_type = config.messaging_type.clone();
            Box::pin(async move {
                if messaging_type != "channel" {
                    return None;
                }

                let channel_config = match mode {
                    EventBusMode::Publisher => ChannelConfig::publisher(),
                    EventBusMode::Subscriber { domain, .. } => ChannelConfig::subscriber(domain),
                    EventBusMode::SubscriberAll { .. } => ChannelConfig::subscriber_all(),
                };

                let bus = ChannelEventBus::new(channel_config);
                info!(messaging_type = "channel", "Event bus initialized");
                Some(Ok(Arc::new(bus) as Arc<dyn EventBus>))
            })
        },
    }
}

#[cfg(test)]
#[path = "mod.test.rs"]
mod tests;
