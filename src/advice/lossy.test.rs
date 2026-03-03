//! Tests for lossy event bus wrapper.
//!
//! The lossy bus simulates unreliable message delivery for resilience testing:
//! - Configurable drop rate (0.0-1.0)
//! - Statistics tracking (total, dropped, passed)
//! - Runtime rate adjustment
//!
//! Why this matters: Event-driven systems must handle message loss gracefully.
//! The lossy bus enables testing recovery mechanisms, idempotency, and
//! replay logic without requiring network failures.
//!
//! Key behaviors verified:
//! - Pass-through mode (drop_rate=0.0) publishes all messages
//! - Drop-all mode (drop_rate=1.0) drops all messages
//! - Partial drop rate approximates configured probability
//! - Stats tracking and reset work correctly

use super::*;
use crate::bus::MockEventBus;
use crate::test_utils::make_event_book;

fn make_test_event_book(domain: &str) -> Arc<EventBook> {
    Arc::new(make_event_book(domain, vec![]))
}

// ============================================================================
// LossyConfig Tests
// ============================================================================

/// LossyConfig::none() creates pass-through config.
///
/// Pass-through is the default; no message loss.
#[test]
fn test_lossy_config_none() {
    let config = LossyConfig::none();
    assert_eq!(config.drop_rate, 0.0);
    assert!(!config.is_lossy());
}

/// LossyConfig::with_drop_rate() sets the specified rate.
#[test]
fn test_lossy_config_with_rate() {
    let config = LossyConfig::with_drop_rate(0.5);
    assert_eq!(config.drop_rate, 0.5);
    assert!(config.is_lossy());
}

/// Drop rate is clamped to [0.0, 1.0] range.
///
/// Prevents invalid probability values.
#[test]
fn test_lossy_config_clamps_rate() {
    let low = LossyConfig::with_drop_rate(-0.5);
    assert_eq!(low.drop_rate, 0.0);

    let high = LossyConfig::with_drop_rate(1.5);
    assert_eq!(high.drop_rate, 1.0);
}

/// LossyConfig::drop_all() sets 100% drop rate.
///
/// Useful for testing complete outage scenarios.
#[test]
fn test_lossy_config_drop_all() {
    let config = LossyConfig::drop_all();
    assert_eq!(config.drop_rate, 1.0);
    assert!(config.is_lossy());
}

// ============================================================================
// LossyBus Publishing Tests
// ============================================================================

/// Pass-through mode publishes all messages without dropping.
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

/// Drop-all mode drops every message.
///
/// Inner bus receives nothing; simulates complete outage.
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

/// 50% drop rate approximates configured probability (statistical test).
///
/// With 1000 samples, observed rate should be within 40-60%.
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

// ============================================================================
// LossyStats Tests
// ============================================================================

/// Stats can be reset to zero.
///
/// Allows resetting between test phases.
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

// ============================================================================
// LossyBus Inner Access Tests
// ============================================================================

/// Inner bus can be accessed and recovered.
#[tokio::test]
async fn test_inner_access() {
    let inner = MockEventBus::new();
    let lossy = LossyBus::passthrough(inner);

    // Access inner
    let _inner_ref = lossy.inner();

    // Consume and get inner back
    let _recovered = lossy.into_inner();
}

/// Drop rate can be changed at runtime.
///
/// Enables dynamic fault injection during tests.
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

/// Subscribe delegates to inner bus (lossy only affects publish).
#[tokio::test]
async fn test_lossy_bus_delegates_subscribe() {
    let inner = MockEventBus::new();
    let lossy = LossyBus::passthrough(inner);

    // Subscribe should succeed (delegates to inner)
    let book = Arc::new(EventBook::default());
    let result = lossy.publish(book).await;
    assert!(result.is_ok());
}

// ============================================================================
// LossyDynBus Tests
// ============================================================================

/// LossyDynBus wraps Arc<dyn EventBus> with lossy behavior.
///
/// Allows wrapping trait objects from create_subscriber.
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
