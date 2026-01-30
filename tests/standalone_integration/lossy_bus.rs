//! Lossy bus integration tests — deterministic event dropping.

use crate::common::*;
use angzarr::bus::{
    ChannelConfig, ChannelEventBus, EventBus, EventHandler, PublishResult, Result as BusResult,
};
use std::sync::atomic::AtomicUsize;

/// Deterministic event bus wrapper that drops events based on a pattern.
/// For integration tests - predictable behavior.
struct DeterministicLossyBus {
    inner: Arc<dyn EventBus>,
    drop_pattern: Vec<bool>, // true = drop, false = pass through
    counter: AtomicUsize,
}

impl DeterministicLossyBus {
    /// Create with a pattern: e.g., [false, true] drops every other event
    fn with_pattern(inner: Arc<dyn EventBus>, pattern: Vec<bool>) -> Self {
        Self {
            inner,
            drop_pattern: pattern,
            counter: AtomicUsize::new(0),
        }
    }

    /// Drop every Nth event (e.g., every_n=2 drops events 1, 3, 5...)
    fn drop_every_nth(inner: Arc<dyn EventBus>, n: usize) -> Self {
        let pattern: Vec<bool> = (0..n).map(|i| i == n - 1).collect();
        Self::with_pattern(inner, pattern)
    }

    /// Drop all events
    fn drop_all(inner: Arc<dyn EventBus>) -> Self {
        Self::with_pattern(inner, vec![true])
    }

    /// Pass all events (no dropping)
    fn passthrough(inner: Arc<dyn EventBus>) -> Self {
        Self::with_pattern(inner, vec![false])
    }
}

#[async_trait]
impl EventBus for DeterministicLossyBus {
    async fn publish(&self, book: Arc<EventBook>) -> BusResult<PublishResult> {
        let idx = self.counter.fetch_add(1, Ordering::SeqCst);
        let pattern_idx = idx % self.drop_pattern.len();

        if self.drop_pattern[pattern_idx] {
            // Drop this event, return empty result
            Ok(PublishResult::default())
        } else {
            self.inner.publish(book).await
        }
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> BusResult<()> {
        self.inner.subscribe(handler).await
    }

    async fn start_consuming(&self) -> BusResult<()> {
        self.inner.start_consuming().await
    }

    async fn create_subscriber(&self, name: &str, domain_filter: Option<&str>) -> BusResult<Arc<dyn EventBus>> {
        self.inner.create_subscriber(name, domain_filter).await
    }
}

#[tokio::test]
async fn test_runtime_with_passthrough_bus() {
    // Create channel bus wrapped in passthrough (no drops)
    let channel_bus = Arc::new(ChannelEventBus::new(ChannelConfig::publisher()));
    let passthrough_bus = Arc::new(DeterministicLossyBus::passthrough(channel_bus.clone()));

    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .with_event_bus(passthrough_bus)
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let event_bus = runtime.event_bus();
    let handler_state = RecordingHandlerState::new();
    let subscriber = event_bus.create_subscriber("test-sub", None).await.unwrap();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();
    subscriber.start_consuming().await.unwrap();

    // Wait for subscriber to be ready
    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = runtime.command_client();

    // Execute commands
    for i in 0..10 {
        let cmd =
            create_test_command("orders", Uuid::new_v4(), format!("lossy-{}", i).as_bytes(), 0);
        client.execute(cmd).await.expect("Command failed");
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    // With passthrough, all events should be received
    let count = handler_state.received_count().await;
    assert_eq!(
        count, 10,
        "All events should be received in passthrough mode"
    );
}

#[tokio::test]
async fn test_runtime_with_deterministic_drops() {
    // Create channel bus that drops every other event (50% deterministic)
    let channel_bus = Arc::new(ChannelEventBus::new(ChannelConfig::publisher()));
    let lossy_bus = Arc::new(DeterministicLossyBus::drop_every_nth(
        channel_bus.clone(),
        2, // Drop every 2nd event
    ));

    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .with_event_bus(lossy_bus)
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let event_bus = runtime.event_bus();
    let handler_state = RecordingHandlerState::new();
    let subscriber = event_bus.create_subscriber("test-sub", None).await.unwrap();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();
    subscriber.start_consuming().await.unwrap();

    // Wait for subscriber to be ready
    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = runtime.command_client();

    // Execute 10 commands
    for i in 0..10 {
        let cmd =
            create_test_command("orders", Uuid::new_v4(), format!("lossy-{}", i).as_bytes(), 0);
        client.execute(cmd).await.expect("Command failed");
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Events should still be persisted (lossy is only for pub/sub, not storage)
    let roots = runtime
        .event_store("orders")
        .unwrap()
        .list_roots("orders", DEFAULT_EDITION)
        .await
        .unwrap();
    assert_eq!(roots.len(), 10, "All events should be persisted to storage");

    // Exactly half should be received (deterministic: drop every 2nd)
    let received = handler_state.received_count().await;
    assert_eq!(
        received, 5,
        "Should receive exactly 5 events (every other dropped)"
    );
}

#[tokio::test]
async fn test_lossy_bus_commands_still_succeed() {
    // Create channel bus that drops ALL events
    let channel_bus = Arc::new(ChannelEventBus::new(ChannelConfig::publisher()));
    let drop_all_bus = Arc::new(DeterministicLossyBus::drop_all(channel_bus.clone()));

    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .with_event_bus(drop_all_bus)
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();

    for i in 0..5 {
        let cmd = create_test_command("orders", Uuid::new_v4(), format!("drop-all-{}", i).as_bytes(), 0);
        let result = client.execute(cmd).await;
        assert!(
            result.is_ok(),
            "Command {} should succeed even with lossy bus",
            i
        );
    }

    // Events should still be persisted to storage
    let roots = runtime
        .event_store("orders")
        .unwrap()
        .list_roots("orders", DEFAULT_EDITION)
        .await
        .unwrap();
    assert_eq!(roots.len(), 5, "Events should still be persisted");
}
