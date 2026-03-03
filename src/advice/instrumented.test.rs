//! Tests for metrics instrumentation wrapper.
//!
//! The Instrumented wrapper adds OpenTelemetry metrics to storage operations:
//! - Duration histograms for all operations
//! - Event/snapshot counters by domain and storage type
//! - Position update counters by handler
//!
//! Why this matters: Observability without polluting business logic.
//! Storage implementations stay pure; metrics are applied at composition time.
//!
//! Key behaviors verified:
//! - Wrapper delegates to inner implementation
//! - Errors propagate unchanged
//!
//! Note: Metric emission tests require integration tests with OTel collector.
//! These unit tests verify the wrapper doesn't break storage behavior.

use super::*;
use crate::storage::MockEventStore;

/// Instrumented wrapper delegates to inner storage.
///
/// All operations pass through; wrapper is transparent.
#[tokio::test]
async fn test_instrumented_delegates_to_inner() {
    let inner = MockEventStore::new();
    let instrumented = Instrumented::new(inner, "mock");

    let root = Uuid::new_v4();

    // Should delegate and succeed
    let events = instrumented.get("test", "angzarr", root).await.unwrap();
    assert!(events.is_empty());
}

/// Errors from inner storage propagate through wrapper.
///
/// Wrapper doesn't swallow or transform errors.
#[tokio::test]
async fn test_instrumented_preserves_errors() {
    let inner = MockEventStore::new();
    inner.set_fail_on_get(true).await;

    let instrumented = Instrumented::new(inner, "mock");
    let root = Uuid::new_v4();

    // Should propagate error
    let result = instrumented.get("test", "angzarr", root).await;
    assert!(result.is_err());
}
