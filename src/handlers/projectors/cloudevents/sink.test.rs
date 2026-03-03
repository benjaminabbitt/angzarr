//! Tests for CloudEvents sink trait and implementations.
//!
//! The sink trait defines the interface for CloudEvents output destinations.
//! Tests verify NullSink (testing/disabled) and MultiSink (fan-out to
//! multiple destinations) behave correctly.

use super::*;
use cloudevents::{EventBuilder, EventBuilderV10};
use std::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// Test Helpers
// ============================================================================

fn test_event(id: &str) -> CloudEventEnvelope {
    EventBuilderV10::new()
        .id(id)
        .ty("test.Event")
        .source("test/source")
        .build()
        .expect("valid test event")
}

struct CountingSink {
    count: AtomicUsize,
}

impl CountingSink {
    fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
        }
    }

    fn count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl CloudEventsSink for CountingSink {
    async fn publish(
        &self,
        events: Vec<CloudEventEnvelope>,
        _format: ContentType,
    ) -> Result<(), SinkError> {
        self.count.fetch_add(events.len(), Ordering::SeqCst);
        Ok(())
    }

    fn name(&self) -> &str {
        "counting"
    }
}

// ============================================================================
// NullSink Tests
// ============================================================================

/// NullSink silently discards events.
///
/// Used for testing or when CloudEvents output is disabled.
#[tokio::test]
async fn test_null_sink() {
    let sink = NullSink;
    let events = vec![test_event("test-1")];
    let result = sink.publish(events, ContentType::Json).await;
    assert!(result.is_ok());
}

// ============================================================================
// MultiSink Tests
// ============================================================================

/// MultiSink fans out to all configured sinks.
///
/// When both HTTP and Kafka are configured, events go to both.
/// Each sink receives the full batch.
#[tokio::test]
async fn test_multi_sink() {
    let sink1 = Arc::new(CountingSink::new());
    let sink2 = Arc::new(CountingSink::new());

    let multi = MultiSink::new(vec![
        sink1.clone() as Arc<dyn CloudEventsSink>,
        sink2.clone() as Arc<dyn CloudEventsSink>,
    ]);

    let events = vec![
        test_event("test-1"),
        test_event("test-2"),
        test_event("test-3"),
    ];

    let result = multi.publish(events, ContentType::Json).await;
    assert!(result.is_ok());
    assert_eq!(sink1.count(), 3);
    assert_eq!(sink2.count(), 3);
}
