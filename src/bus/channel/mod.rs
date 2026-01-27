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

    /// Extract domain from event book.
    fn get_domain(book: &EventBook) -> &str {
        book.cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown")
    }

    /// Start consuming messages (call after subscribe).
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
        let domain_filter = self.config.domain_filter.clone();

        // Spawn consumer task
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(book) => {
                        let domain = Self::get_domain(&book);

                        // Check domain filter (hierarchical matching)
                        let matches = match &domain_filter {
                            None => true,
                            Some(filter) => domain_matches(domain, filter),
                        };

                        if !matches {
                            continue;
                        }

                        debug!(
                            domain = %domain,
                            "Received event book via channel"
                        );

                        // Call all handlers
                        let handlers_guard = handlers.read().await;
                        for handler in handlers_guard.iter() {
                            if let Err(e) = handler.handle(Arc::clone(&book)).await {
                                error!(error = %e, "Handler failed");
                            }
                        }
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
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        let domain = Self::get_domain(&book).to_string();

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::BusError;
    use crate::proto::{Cover, Uuid as ProtoUuid};
    use futures::future::BoxFuture;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    fn make_event_book(domain: &str, root: Uuid) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
            }),
            pages: vec![],
            snapshot: None,
            snapshot_state: None,
        }
    }

    struct CountingHandler {
        count: Arc<AtomicUsize>,
    }

    impl EventHandler for CountingHandler {
        fn handle(
            &self,
            _book: Arc<EventBook>,
        ) -> BoxFuture<'static, std::result::Result<(), BusError>> {
            let count = self.count.clone();
            Box::pin(async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }
    }

    #[test]
    fn test_domain_matches_exact() {
        assert!(domain_matches("orders", "orders"));
        assert!(!domain_matches("orders", "inventory"));
    }

    #[test]
    fn test_domain_matches_wildcard() {
        assert!(domain_matches("orders", "#"));
        assert!(domain_matches("anything", "#"));
    }

    #[test]
    fn test_domain_matches_hierarchical() {
        assert!(domain_matches("orders.items", "orders"));
        assert!(domain_matches("orders.items.details", "orders"));
        assert!(!domain_matches("orders", "orders.items"));
        assert!(!domain_matches("ordersextra", "orders")); // No dot separator
    }

    #[tokio::test]
    async fn test_channel_publish_no_receivers() {
        let bus = ChannelEventBus::publisher();
        let book = Arc::new(make_event_book("orders", Uuid::new_v4()));

        // Should not error even with no receivers
        let result = bus.publish(book).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_channel_subscribe_and_receive() {
        let bus = ChannelEventBus::subscriber_all();
        let count = Arc::new(AtomicUsize::new(0));

        // Subscribe handler
        let handler = CountingHandler {
            count: count.clone(),
        };
        bus.subscribe(Box::new(handler)).await.unwrap();
        bus.start_consuming().await.unwrap();

        // Give consumer time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Publish
        let book = Arc::new(make_event_book("orders", Uuid::new_v4()));
        bus.publish(book).await.unwrap();

        // Give handler time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_channel_domain_filter() {
        let bus = ChannelEventBus::subscriber("orders");
        let count = Arc::new(AtomicUsize::new(0));

        let handler = CountingHandler {
            count: count.clone(),
        };
        bus.subscribe(Box::new(handler)).await.unwrap();
        bus.start_consuming().await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Publish to matching domain
        let book1 = Arc::new(make_event_book("orders", Uuid::new_v4()));
        bus.publish(book1).await.unwrap();

        // Publish to non-matching domain
        let book2 = Arc::new(make_event_book("inventory", Uuid::new_v4()));
        bus.publish(book2).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Should only count the matching one
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_channel_shared_sender() {
        let publisher = ChannelEventBus::publisher();
        let subscriber = publisher.with_config(ChannelConfig::subscriber_all());

        let count = Arc::new(AtomicUsize::new(0));
        let handler = CountingHandler {
            count: count.clone(),
        };
        subscriber.subscribe(Box::new(handler)).await.unwrap();
        subscriber.start_consuming().await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Publish via publisher, receive via subscriber
        let book = Arc::new(make_event_book("orders", Uuid::new_v4()));
        publisher.publish(book).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
