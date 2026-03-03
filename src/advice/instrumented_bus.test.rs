//! Tests for event bus instrumentation wrapper.
//!
//! InstrumentedBus adds metrics to EventBus operations:
//! - Publish duration histogram by bus type, domain, outcome
//! - Publish counter by bus type, domain, outcome
//!
//! Why this matters: Bus metrics reveal bottlenecks and failures
//! in message delivery without modifying bus implementations.
//!
//! Key behaviors verified:
//! - Wrapper delegates publish to inner bus
//! - Errors propagate unchanged
//!
//! Note: Metric emission tests require integration tests with OTel collector.

use super::*;
use crate::bus::MockEventBus;
use crate::proto::EventBook;

/// InstrumentedBus delegates publish to inner bus.
#[tokio::test]
async fn test_instrumented_bus_delegates_publish() {
    let inner = MockEventBus::new();
    let bus = InstrumentedBus::new(inner, "mock");

    let book = Arc::new(EventBook::default());
    let result = bus.publish(book).await;
    assert!(result.is_ok());
}

/// Errors from inner bus propagate through wrapper.
///
/// Wrapper doesn't swallow or transform errors.
#[tokio::test]
async fn test_instrumented_bus_propagates_errors() {
    let inner = MockEventBus::new();
    inner.set_fail_on_publish(true).await;

    let bus = InstrumentedBus::new(inner, "mock");
    let book = Arc::new(EventBook::default());
    let result = bus.publish(book).await;
    assert!(result.is_err());
}
