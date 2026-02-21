//! In-memory channel-based event bus for standalone mode.
//!
//! Uses tokio broadcast channels for pub/sub within a single process.
//! Ideal for local development and testing without external dependencies.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info};

use super::{EventBus, EventHandler, PublishResult, Result};
use crate::proto::EventBook;
use crate::proto_ext::CoverExt;

/// Channel capacity for broadcast.
const CHANNEL_CAPACITY: usize = 1024;

/// Configuration for channel event bus.
#[derive(Clone, Debug, Default)]
pub struct ChannelConfig {
    /// Domain filter for subscribers.
    /// - `None` or `Some("#")` matches all domains
    /// - `Some("orders")` matches only "orders" domain
    pub domain_filter: Option<String>,
}

impl ChannelConfig {
    /// Create config for publishing only.
    pub fn publisher() -> Self {
        Self {
            domain_filter: None,
        }
    }

    /// Create config for subscribing to a specific domain.
    pub fn subscriber(domain: impl Into<String>) -> Self {
        Self {
            domain_filter: Some(domain.into()),
        }
    }

    /// Create config for subscribing to all domains.
    pub fn subscriber_all() -> Self {
        Self {
            domain_filter: Some("#".to_string()),
        }
    }
}

/// Check if a domain matches a filter pattern.
///
/// Matching rules:
/// - "#" matches all domains
/// - Exact match: "orders" matches "orders"
/// - Hierarchical: "orders" matches "orders.items" (prefix match with dot separator)
fn domain_matches(domain: &str, filter: &str) -> bool {
    if filter == "#" {
        return true;
    }
    if domain == filter {
        return true;
    }
    // Hierarchical match: filter is prefix of domain with dot separator
    domain.starts_with(filter) && domain[filter.len()..].starts_with('.')
}

/// In-memory event bus using tokio broadcast channels.
///
/// Events are published to a broadcast channel and received by all subscribers.
/// Domain filtering is done on the subscriber side.
pub struct ChannelEventBus {
    /// Broadcast sender for publishing events.
    sender: broadcast::Sender<Arc<EventBook>>,
    /// Configuration including domain filter.
    config: ChannelConfig,
    /// Registered event handlers.
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    /// Flag indicating if consumer task is running.
    consuming: Arc<RwLock<bool>>,
}

impl ChannelEventBus {
    /// Create a new channel event bus.
    pub fn new(config: ChannelConfig) -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);

        info!(
            domain_filter = ?config.domain_filter,
            "Channel event bus initialized"
        );

        Self {
            sender,
            config,
            handlers: Arc::new(RwLock::new(Vec::new())),
            consuming: Arc::new(RwLock::new(false)),
        }
    }

    /// Create a publisher-only bus instance.
    pub fn publisher() -> Self {
        Self::new(ChannelConfig::publisher())
    }

    /// Create a subscriber bus for a specific domain.
    pub fn subscriber(domain: impl Into<String>) -> Self {
        Self::new(ChannelConfig::subscriber(domain))
    }

    /// Create a subscriber bus for all domains.
    pub fn subscriber_all() -> Self {
        Self::new(ChannelConfig::subscriber_all())
    }

    /// Get a clone of the sender for creating linked subscribers.
    pub fn sender(&self) -> broadcast::Sender<Arc<EventBook>> {
        self.sender.clone()
    }

    /// Create a new bus that shares the same channel but has different config.
    pub fn with_config(&self, config: ChannelConfig) -> Self {
        Self {
            sender: self.sender.clone(),
            config,
            handlers: Arc::new(RwLock::new(Vec::new())),
            consuming: Arc::new(RwLock::new(false)),
        }
    }

    /// Start consuming messages (call after subscribe).
    async fn start_consuming_impl(&self) -> Result<()> {
        // Check if already consuming
        {
            let mut consuming = self.consuming.write().await;
            if *consuming {
                return Ok(());
            }
            *consuming = true;
        }

        let mut receiver = self.sender.subscribe();
        let handlers = self.handlers.clone();
        let domain_filter = self.config.domain_filter.clone();

        // Spawn consumer task
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(book) => {
                        let routing_key = book.routing_key();

                        // Check domain filter using routing key (hierarchical matching)
                        let matches = match &domain_filter {
                            None => true,
                            Some(filter) => domain_matches(&routing_key, filter),
                        };

                        if !matches {
                            continue;
                        }

                        debug!(
                            routing_key = %routing_key,
                            "Received event book via channel"
                        );

                        // Call all handlers
                        super::dispatch_to_handlers(&handlers, &book).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        error!(skipped = n, "Channel consumer lagged, skipped messages");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Channel closed, stopping consumer");
                        break;
                    }
                }
            }
        });

        info!(
            domain_filter = ?self.config.domain_filter,
            "Channel consumer started"
        );

        Ok(())
    }
}

#[async_trait]
impl EventBus for ChannelEventBus {
    #[tracing::instrument(name = "bus.publish", skip_all, fields(domain = %book.domain()))]
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        let domain = book.domain().to_string();

        // Send to channel (ignore error if no receivers)
        match self.sender.send(book) {
            Ok(receiver_count) => {
                debug!(
                    domain = %domain,
                    receivers = receiver_count,
                    "Published event book to channel"
                );
            }
            Err(_) => {
                // No receivers, that's okay for publish-only scenarios
                debug!(domain = %domain, "Published event book (no receivers)");
            }
        }

        // Channel bus is async-only, no synchronous projections
        Ok(PublishResult::default())
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        let count = {
            let mut handlers = self.handlers.write().await;
            handlers.push(handler);
            handlers.len()
        };

        info!(handler_count = count, "Handler subscribed to channel bus");

        Ok(())
    }

    async fn start_consuming(&self) -> Result<()> {
        self.start_consuming_impl().await
    }

    async fn create_subscriber(
        &self,
        _name: &str,
        domain_filter: Option<&str>,
    ) -> Result<Arc<dyn EventBus>> {
        let config = match domain_filter {
            Some(d) => ChannelConfig::subscriber(d),
            None => ChannelConfig::subscriber_all(),
        };
        Ok(Arc::new(self.with_config(config)))
    }
}

#[cfg(test)]
mod tests;
