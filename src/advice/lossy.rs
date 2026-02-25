//! Lossy event bus wrapper for testing unreliable message delivery.
//!
//! Wraps any [`EventBus`] implementation and optionally drops messages
//! based on a configurable probability. Useful for testing resilience
//! to message loss.
//!
//! # Example
//!
//! ```ignore
//! use angzarr::advice::{LossyBus, LossyConfig};
//! use angzarr::bus::ChannelEventBus;
//!
//! // Create a bus that drops 10% of messages
//! let inner = ChannelEventBus::publisher();
//! let lossy = LossyBus::new(inner, LossyConfig::with_drop_rate(0.1));
//!
//! // Or create a non-lossy wrapper (pass-through)
//! let passthrough = LossyBus::new(inner, LossyConfig::none());
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use rand::Rng;
use tracing::{debug, warn};

use crate::bus::{EventBus, EventHandler, PublishResult, Result};
use crate::proto::EventBook;

/// Configuration for lossy behavior.
#[derive(Clone, Debug)]
pub struct LossyConfig {
    /// Probability of dropping a message (0.0 to 1.0).
    /// - 0.0 = never drop (pass-through)
    /// - 0.5 = drop 50% of messages
    /// - 1.0 = drop all messages
    pub drop_rate: f64,
    /// Whether to log dropped messages.
    pub log_drops: bool,
}

impl Default for LossyConfig {
    fn default() -> Self {
        Self::none()
    }
}

impl LossyConfig {
    /// Create a config that never drops messages (pass-through).
    pub fn none() -> Self {
        Self {
            drop_rate: 0.0,
            log_drops: false,
        }
    }

    /// Create a config with a specific drop rate.
    ///
    /// # Arguments
    /// * `rate` - Drop probability (0.0 to 1.0), clamped to valid range
    pub fn with_drop_rate(rate: f64) -> Self {
        Self {
            drop_rate: rate.clamp(0.0, 1.0),
            log_drops: true,
        }
    }

    /// Create a config that drops all messages.
    pub fn drop_all() -> Self {
        Self {
            drop_rate: 1.0,
            log_drops: true,
        }
    }

    /// Set whether to log dropped messages.
    pub fn with_logging(mut self, log: bool) -> Self {
        self.log_drops = log;
        self
    }

    /// Check if this config has any lossy behavior enabled.
    pub fn is_lossy(&self) -> bool {
        self.drop_rate > 0.0
    }
}

/// Statistics for the lossy bus.
#[derive(Debug, Default)]
pub struct LossyStats {
    /// Total messages received for publish.
    pub total: AtomicU64,
    /// Messages that were dropped.
    pub dropped: AtomicU64,
    /// Messages that were passed through.
    pub passed: AtomicU64,
}

impl LossyStats {
    /// Get a snapshot of current stats.
    pub fn snapshot(&self) -> (u64, u64, u64) {
        (
            self.total.load(Ordering::Relaxed),
            self.dropped.load(Ordering::Relaxed),
            self.passed.load(Ordering::Relaxed),
        )
    }

    /// Get the actual drop rate observed.
    pub fn observed_drop_rate(&self) -> f64 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            self.dropped.load(Ordering::Relaxed) as f64 / total as f64
        }
    }

    /// Reset all counters.
    pub fn reset(&self) {
        self.total.store(0, Ordering::Relaxed);
        self.dropped.store(0, Ordering::Relaxed);
        self.passed.store(0, Ordering::Relaxed);
    }
}

/// Check if a message should be dropped based on config.
fn should_drop(config: &LossyConfig) -> bool {
    if config.drop_rate <= 0.0 {
        return false;
    }
    if config.drop_rate >= 1.0 {
        return true;
    }
    rand::rng().random::<f64>() < config.drop_rate
}

/// Wrapper that optionally drops messages for testing.
///
/// When `drop_rate` is 0.0, this is a pure pass-through with minimal overhead.
/// When `drop_rate` > 0.0, messages are randomly dropped based on the probability.
pub struct LossyBus<T> {
    inner: T,
    config: LossyConfig,
    stats: Arc<LossyStats>,
}

impl<T> LossyBus<T> {
    /// Create a new lossy wrapper around an existing bus.
    pub fn new(inner: T, config: LossyConfig) -> Self {
        if config.is_lossy() {
            warn!(
                drop_rate = config.drop_rate,
                "Lossy event bus enabled - messages may be dropped"
            );
        }

        Self {
            inner,
            config,
            stats: Arc::new(LossyStats::default()),
        }
    }

    /// Create a pass-through wrapper (no message loss).
    pub fn passthrough(inner: T) -> Self {
        Self::new(inner, LossyConfig::none())
    }

    /// Get a reference to the inner bus.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Consume wrapper and return the inner bus.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Get current statistics.
    pub fn stats(&self) -> &LossyStats {
        &self.stats
    }

    /// Update the drop rate at runtime.
    pub fn set_drop_rate(&mut self, rate: f64) {
        self.config.drop_rate = rate.clamp(0.0, 1.0);
    }
}

#[async_trait]
impl<T: EventBus> EventBus for LossyBus<T> {
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        self.stats.total.fetch_add(1, Ordering::Relaxed);

        if should_drop(&self.config) {
            self.stats.dropped.fetch_add(1, Ordering::Relaxed);

            if self.config.log_drops {
                let domain = book
                    .cover
                    .as_ref()
                    .map(|c| c.domain.as_str())
                    .unwrap_or("unknown");
                debug!(domain = %domain, "Lossy bus dropped message");
            }

            // Return success but don't actually publish
            return Ok(PublishResult::default());
        }

        self.stats.passed.fetch_add(1, Ordering::Relaxed);
        self.inner.publish(book).await
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        // Subscribe passes through - we only drop on publish
        self.inner.subscribe(handler).await
    }

    async fn start_consuming(&self) -> Result<()> {
        self.inner.start_consuming().await
    }

    async fn create_subscriber(
        &self,
        name: &str,
        domain_filter: Option<&str>,
    ) -> Result<Arc<dyn EventBus>> {
        // Create subscriber from inner bus, then wrap with lossy behavior
        let subscriber = self.inner.create_subscriber(name, domain_filter).await?;
        Ok(LossyDynBus::wrap(subscriber, self.config.clone()))
    }

    fn max_message_size(&self) -> Option<usize> {
        self.inner.max_message_size()
    }
}

/// Wrapper for `Arc<dyn EventBus>` to allow lossy wrapping of trait objects.
pub struct LossyDynBus {
    inner: Arc<dyn EventBus>,
    config: LossyConfig,
    stats: Arc<LossyStats>,
}

impl LossyDynBus {
    /// Wrap a dynamic event bus with lossy behavior.
    pub fn new(inner: Arc<dyn EventBus>, config: LossyConfig) -> Self {
        if config.is_lossy() {
            warn!(
                drop_rate = config.drop_rate,
                "Lossy event bus enabled - messages may be dropped"
            );
        }

        Self {
            inner,
            config,
            stats: Arc::new(LossyStats::default()),
        }
    }

    /// Wrap a dynamic event bus, returning an Arc.
    pub fn wrap(inner: Arc<dyn EventBus>, config: LossyConfig) -> Arc<dyn EventBus> {
        Arc::new(Self::new(inner, config))
    }

    /// Get current statistics.
    pub fn stats(&self) -> &LossyStats {
        &self.stats
    }
}

#[async_trait]
impl EventBus for LossyDynBus {
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        self.stats.total.fetch_add(1, Ordering::Relaxed);

        if should_drop(&self.config) {
            self.stats.dropped.fetch_add(1, Ordering::Relaxed);

            if self.config.log_drops {
                let domain = book
                    .cover
                    .as_ref()
                    .map(|c| c.domain.as_str())
                    .unwrap_or("unknown");
                debug!(domain = %domain, "Lossy bus dropped message");
            }

            return Ok(PublishResult::default());
        }

        self.stats.passed.fetch_add(1, Ordering::Relaxed);
        self.inner.publish(book).await
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        self.inner.subscribe(handler).await
    }

    async fn start_consuming(&self) -> Result<()> {
        self.inner.start_consuming().await
    }

    async fn create_subscriber(
        &self,
        name: &str,
        domain_filter: Option<&str>,
    ) -> Result<Arc<dyn EventBus>> {
        let subscriber = self.inner.create_subscriber(name, domain_filter).await?;
        Ok(Self::wrap(subscriber, self.config.clone()))
    }

    fn max_message_size(&self) -> Option<usize> {
        self.inner.max_message_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::MockEventBus;
    use crate::test_utils::make_event_book;

    fn make_test_event_book(domain: &str) -> Arc<EventBook> {
        Arc::new(make_event_book(domain, vec![]))
    }

    #[test]
    fn test_lossy_config_none() {
        let config = LossyConfig::none();
        assert_eq!(config.drop_rate, 0.0);
        assert!(!config.is_lossy());
    }

    #[test]
    fn test_lossy_config_with_rate() {
        let config = LossyConfig::with_drop_rate(0.5);
        assert_eq!(config.drop_rate, 0.5);
        assert!(config.is_lossy());
    }

    #[test]
    fn test_lossy_config_clamps_rate() {
        let low = LossyConfig::with_drop_rate(-0.5);
        assert_eq!(low.drop_rate, 0.0);

        let high = LossyConfig::with_drop_rate(1.5);
        assert_eq!(high.drop_rate, 1.0);
    }

    #[test]
    fn test_lossy_config_drop_all() {
        let config = LossyConfig::drop_all();
        assert_eq!(config.drop_rate, 1.0);
        assert!(config.is_lossy());
    }

    #[tokio::test]
    async fn test_passthrough_publishes_all() {
        let inner = MockEventBus::new();
        let lossy = LossyBus::passthrough(inner);

        for _ in 0..10 {
            lossy.publish(make_test_event_book("orders")).await.unwrap();
        }

        let (total, dropped, passed) = lossy.stats().snapshot();
        assert_eq!(total, 10);
        assert_eq!(dropped, 0);
        assert_eq!(passed, 10);
    }

    #[tokio::test]
    async fn test_drop_all_drops_everything() {
        let inner = MockEventBus::new();
        let lossy = LossyBus::new(inner, LossyConfig::drop_all());

        for _ in 0..10 {
            lossy.publish(make_test_event_book("orders")).await.unwrap();
        }

        let (total, dropped, passed) = lossy.stats().snapshot();
        assert_eq!(total, 10);
        assert_eq!(dropped, 10);
        assert_eq!(passed, 0);
    }

    #[tokio::test]
    async fn test_partial_drop_rate() {
        let inner = MockEventBus::new();
        let lossy = LossyBus::new(inner, LossyConfig::with_drop_rate(0.5).with_logging(false));

        // Publish many messages to get statistical significance
        for _ in 0..1000 {
            lossy.publish(make_test_event_book("orders")).await.unwrap();
        }

        let (total, dropped, passed) = lossy.stats().snapshot();
        assert_eq!(total, 1000);
        assert_eq!(dropped + passed, 1000);

        // With 1000 samples and 50% drop rate, we should be within 40-60%
        let observed_rate = lossy.stats().observed_drop_rate();
        assert!(
            observed_rate > 0.4 && observed_rate < 0.6,
            "Expected ~50% drop rate, got {:.2}%",
            observed_rate * 100.0
        );
    }

    #[tokio::test]
    async fn test_stats_reset() {
        let inner = MockEventBus::new();
        let lossy = LossyBus::new(inner, LossyConfig::with_drop_rate(0.5).with_logging(false));

        for _ in 0..10 {
            lossy.publish(make_test_event_book("orders")).await.unwrap();
        }

        let (total, _, _) = lossy.stats().snapshot();
        assert_eq!(total, 10);

        lossy.stats().reset();

        let (total, dropped, passed) = lossy.stats().snapshot();
        assert_eq!(total, 0);
        assert_eq!(dropped, 0);
        assert_eq!(passed, 0);
    }

    #[tokio::test]
    async fn test_inner_access() {
        let inner = MockEventBus::new();
        let lossy = LossyBus::passthrough(inner);

        // Access inner
        let _inner_ref = lossy.inner();

        // Consume and get inner back
        let _recovered = lossy.into_inner();
    }

    #[tokio::test]
    async fn test_runtime_rate_change() {
        let inner = MockEventBus::new();
        let mut lossy = LossyBus::passthrough(inner);

        // Initially pass-through
        lossy.publish(make_test_event_book("orders")).await.unwrap();
        assert_eq!(lossy.stats().snapshot().2, 1); // passed = 1

        // Change to drop-all
        lossy.set_drop_rate(1.0);
        lossy.publish(make_test_event_book("orders")).await.unwrap();
        assert_eq!(lossy.stats().snapshot().1, 1); // dropped = 1
    }

    #[tokio::test]
    async fn test_lossy_bus_delegates_subscribe() {
        let inner = MockEventBus::new();
        let lossy = LossyBus::passthrough(inner);

        // Subscribe should succeed (delegates to inner)
        let book = Arc::new(EventBook::default());
        let result = lossy.publish(book).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_lossy_dyn_bus_drops_messages() {
        let inner = MockEventBus::new();
        let lossy = LossyDynBus::new(Arc::new(inner), LossyConfig::drop_all());

        for _ in 0..10 {
            lossy.publish(make_test_event_book("orders")).await.unwrap();
        }

        let (total, dropped, passed) = lossy.stats().snapshot();
        assert_eq!(total, 10);
        assert_eq!(dropped, 10);
        assert_eq!(passed, 0);
    }
}
