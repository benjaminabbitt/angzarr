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

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info};

use super::{CommandBus, CommandHandler, EventBus, EventHandler, PublishResult, Result};
use crate::proto::{CommandBook, EventBook};
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

// ============================================================================
// Command Bus
// ============================================================================

use std::collections::HashMap;

/// In-memory command bus using tokio broadcast channels.
///
/// Commands are published to a broadcast channel and routed to handlers
/// based on their domain. Used for async command execution in standalone mode.
pub struct ChannelCommandBus {
    /// Broadcast sender for publishing commands.
    sender: broadcast::Sender<Arc<CommandBook>>,
    /// Handlers by domain.
    handlers: Arc<RwLock<HashMap<String, Box<dyn CommandHandler>>>>,
    /// Flag indicating if consumer task is running.
    consuming: Arc<RwLock<bool>>,
}

impl ChannelCommandBus {
    /// Create a new channel command bus.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);

        info!("Channel command bus initialized");

        Self {
            sender,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            consuming: Arc::new(RwLock::new(false)),
        }
    }

    /// Start consuming commands (call after subscribe).
    pub async fn start_consuming(&self) -> Result<()> {
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

        // Spawn consumer task
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(command) => {
                        let domain = command
                            .cover
                            .as_ref()
                            .map(|c| c.domain.clone())
                            .unwrap_or_else(|| "unknown".to_string());

                        debug!(
                            domain = %domain,
                            "Received command via channel"
                        );

                        // Find handler for domain
                        let handlers = handlers.read().await;
                        if let Some(handler) = handlers.get(&domain) {
                            if let Err(e) = handler.handle(command).await {
                                error!(
                                    domain = %domain,
                                    error = %e,
                                    "Command handler failed"
                                );
                            }
                        } else {
                            error!(
                                domain = %domain,
                                "No command handler registered for domain"
                            );
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        error!(
                            skipped = n,
                            "Command channel consumer lagged, skipped messages"
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Command channel closed, stopping consumer");
                        break;
                    }
                }
            }
        });

        info!("Channel command consumer started");

        Ok(())
    }
}

impl Default for ChannelCommandBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CommandBus for ChannelCommandBus {
    #[tracing::instrument(name = "command_bus.publish", skip_all, fields(domain = %command.cover.as_ref().map(|c| c.domain.as_str()).unwrap_or("unknown")))]
    async fn publish(&self, command: Arc<CommandBook>) -> Result<()> {
        let domain = command
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Send to channel (ignore error if no receivers)
        match self.sender.send(command) {
            Ok(receiver_count) => {
                debug!(
                    domain = %domain,
                    receivers = receiver_count,
                    "Published command to channel"
                );
            }
            Err(_) => {
                // No receivers - this is an error for commands
                error!(domain = %domain, "Failed to publish command (no receivers)");
            }
        }

        Ok(())
    }

    async fn subscribe(&self, domain: &str, handler: Box<dyn CommandHandler>) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        handlers.insert(domain.to_string(), handler);

        info!(domain = %domain, "Command handler subscribed to channel bus");

        Ok(())
    }
}

#[cfg(test)]
mod tests;
