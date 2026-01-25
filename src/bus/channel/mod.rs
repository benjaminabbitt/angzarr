//! In-memory channel-based event bus for standalone mode.
//!
//! Uses tokio broadcast channels for pub/sub within a single process.
//! Ideal for local development and testing without external dependencies.
//! DLQ support via separate broadcast channel.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

use super::{DeadLetterHandler, DlqConfig, EventBus, EventHandler, FailedMessage, PublishResult, Result};
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
    /// Dead letter queue configuration.
    pub dlq: DlqConfig,
}

impl ChannelConfig {
    /// Create config for publishing only.
    pub fn publisher() -> Self {
        Self {
            domain_filter: None,
            dlq: DlqConfig::default(),
        }
    }

    /// Create config for subscribing to a specific domain.
    pub fn subscriber(domain: impl Into<String>) -> Self {
        Self {
            domain_filter: Some(domain.into()),
            dlq: DlqConfig::default(),
        }
    }

    /// Create config for subscribing to all domains.
    pub fn subscriber_all() -> Self {
        Self {
            domain_filter: Some("#".to_string()),
            dlq: DlqConfig::default(),
        }
    }

    /// Set custom DLQ configuration.
    pub fn with_dlq(mut self, dlq: DlqConfig) -> Self {
        self.dlq = dlq;
        self
    }
}

/// In-memory event bus using tokio broadcast channels.
///
/// Events are published to a broadcast channel and received by all subscribers.
/// Domain filtering is done on the subscriber side.
/// Failed messages go to a separate DLQ channel.
pub struct ChannelEventBus {
    /// Broadcast sender for publishing events.
    sender: broadcast::Sender<Arc<EventBook>>,
    /// Broadcast sender for DLQ messages.
    dlq_sender: broadcast::Sender<FailedMessage>,
    /// Configuration including domain filter.
    config: ChannelConfig,
    /// Registered event handlers.
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    /// Registered DLQ handlers.
    dlq_handlers: Arc<RwLock<Vec<Box<dyn DeadLetterHandler>>>>,
    /// Flag indicating if consumer task is running.
    consuming: Arc<RwLock<bool>>,
}

impl ChannelEventBus {
    /// Create a new channel event bus.
    pub fn new(config: ChannelConfig) -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (dlq_sender, _) = broadcast::channel(CHANNEL_CAPACITY);

        info!(
            domain_filter = ?config.domain_filter,
            dlq_enabled = config.dlq.enabled,
            "Channel event bus initialized"
        );

        Self {
            sender,
            dlq_sender,
            config,
            handlers: Arc::new(RwLock::new(Vec::new())),
            dlq_handlers: Arc::new(RwLock::new(Vec::new())),
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
            dlq_sender: self.dlq_sender.clone(),
            config,
            handlers: Arc::new(RwLock::new(Vec::new())),
            dlq_handlers: Arc::new(RwLock::new(Vec::new())),
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
        let dlq_config = self.config.dlq.clone();
        let dlq_sender = self.dlq_sender.clone();
        let sender = self.sender.clone();

        // Spawn consumer task
        tokio::spawn(async move {
            // Track retry counts per message (using correlation_id as key)
            let retry_counts: Arc<RwLock<std::collections::HashMap<String, u32>>> =
                Arc::new(RwLock::new(std::collections::HashMap::new()));

            loop {
                match receiver.recv().await {
                    Ok(book) => {
                        let domain = Self::get_domain(&book);

                        // Check domain filter (hierarchical matching)
                        let matches = match &domain_filter {
                            None => true,
                            Some(filter) => crate::bus::domain_matches(domain, filter),
                        };

                        if !matches {
                            continue;
                        }

                        // Get retry count for this message
                        let message_key = book.correlation_id.clone();
                        let retry_count = {
                            let counts = retry_counts.read().await;
                            counts.get(&message_key).copied().unwrap_or(0)
                        };

                        debug!(
                            domain = %domain,
                            retry_count = retry_count,
                            "Received event book via channel"
                        );

                        // Call all handlers, tracking failures
                        let handlers_guard = handlers.read().await;
                        let mut all_succeeded = true;
                        let mut last_error = String::new();
                        let mut failed_handler = String::new();

                        for (idx, handler) in handlers_guard.iter().enumerate() {
                            if let Err(e) = handler.handle(Arc::clone(&book)).await {
                                all_succeeded = false;
                                last_error = e.to_string();
                                failed_handler = format!("handler-{}", idx);
                                error!(
                                    error = %e,
                                    retry_count = retry_count,
                                    "Handler failed"
                                );
                                break;
                            }
                        }
                        drop(handlers_guard);

                        if all_succeeded {
                            // Clear retry count on success
                            let mut counts = retry_counts.write().await;
                            counts.remove(&message_key);
                        } else if dlq_config.should_retry(retry_count) {
                            // Retry: wait for backoff then republish
                            let delay = dlq_config.backoff_delay(retry_count);
                            warn!(
                                retry_count = retry_count,
                                delay_ms = delay.as_millis() as u64,
                                "Handler failed, scheduling retry"
                            );

                            // Increment retry count
                            {
                                let mut counts = retry_counts.write().await;
                                counts.insert(message_key.clone(), retry_count + 1);
                            }

                            // Schedule retry after backoff
                            let sender_clone = sender.clone();
                            let book_clone = book.clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(delay).await;
                                let _ = sender_clone.send(book_clone);
                            });
                        } else {
                            // Max retries exceeded: send to DLQ
                            error!(
                                retry_count = retry_count,
                                error = %last_error,
                                "Max retries exceeded, sending to DLQ"
                            );

                            // Clear retry count
                            {
                                let mut counts = retry_counts.write().await;
                                counts.remove(&message_key);
                            }

                            if dlq_config.enabled {
                                let failed_msg = FailedMessage::new(
                                    &book,
                                    &last_error,
                                    &failed_handler,
                                    retry_count + 1,
                                );
                                let _ = dlq_sender.send(failed_msg);
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

    /// Get the DLQ sender for sharing with linked buses.
    pub fn dlq_sender(&self) -> broadcast::Sender<FailedMessage> {
        self.dlq_sender.clone()
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

    async fn send_to_dlq(&self, message: FailedMessage) -> Result<()> {
        if !self.config.dlq.enabled {
            warn!(
                domain = %message.domain,
                root_id = %message.root_id,
                "DLQ disabled, dropping failed message"
            );
            return Ok(());
        }

        match self.dlq_sender.send(message.clone()) {
            Ok(receiver_count) => {
                info!(
                    domain = %message.domain,
                    root_id = %message.root_id,
                    handler = %message.handler_name,
                    attempts = message.attempt_count,
                    receivers = receiver_count,
                    "Message sent to DLQ"
                );
            }
            Err(_) => {
                // No receivers - still succeeds (message is "stored" in the channel)
                debug!(
                    domain = %message.domain,
                    "Message sent to DLQ (no receivers)"
                );
            }
        }

        Ok(())
    }

    async fn subscribe_dlq(&self, handler: Box<dyn DeadLetterHandler>) -> Result<()> {
        use super::BusError;

        if !self.config.dlq.enabled {
            return Err(BusError::DeadLetterQueue("DLQ is disabled".to_string()));
        }

        // Add handler
        {
            let mut handlers = self.dlq_handlers.write().await;
            handlers.push(handler);
        }

        // Start DLQ consumer
        let mut receiver = self.dlq_sender.subscribe();
        let dlq_handlers = self.dlq_handlers.clone();

        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(failed_msg) => {
                        debug!(
                            domain = %failed_msg.domain,
                            "Received message from DLQ"
                        );

                        let handlers_guard = dlq_handlers.read().await;
                        for handler in handlers_guard.iter() {
                            if let Err(e) = handler.handle_dlq(failed_msg.clone()).await {
                                error!(error = %e, "DLQ handler failed");
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        error!(skipped = n, "DLQ consumer lagged, skipped messages");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("DLQ channel closed, stopping consumer");
                        break;
                    }
                }
            }
        });

        info!("DLQ consumer started");
        Ok(())
    }

    fn dlq_config(&self) -> DlqConfig {
        self.config.dlq.clone()
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
            }),
            pages: vec![],
            snapshot: None,
            correlation_id: String::new(),
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
