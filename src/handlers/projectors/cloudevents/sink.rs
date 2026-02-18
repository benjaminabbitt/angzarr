//! CloudEvents sink trait and errors.
//!
//! Defines the interface for CloudEvents output destinations
//! (HTTP webhooks, Kafka, etc.).

use super::types::CloudEventEnvelope;
use async_trait::async_trait;
use std::sync::Arc;

/// Errors that can occur when publishing CloudEvents.
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Kafka error.
    #[cfg(feature = "kafka")]
    #[error("Kafka error: {0}")]
    Kafka(String),

    /// JSON serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Sink is not configured/available.
    #[error("Sink unavailable: {0}")]
    Unavailable(String),
}

/// Trait for CloudEvents output destinations.
///
/// Implementations handle batching, retries, and protocol-specific formatting.
#[async_trait]
pub trait CloudEventsSink: Send + Sync {
    /// Publish a batch of CloudEvents.
    ///
    /// Implementations should handle retries internally for transient failures.
    async fn publish(&self, events: Vec<CloudEventEnvelope>) -> Result<(), SinkError>;

    /// Return the sink name for logging/metrics.
    fn name(&self) -> &str;
}

/// Multi-sink that fans out to multiple destinations.
///
/// Used when both HTTP and Kafka are configured.
pub struct MultiSink {
    sinks: Vec<Arc<dyn CloudEventsSink>>,
}

impl MultiSink {
    /// Create a new multi-sink from a list of sinks.
    pub fn new(sinks: Vec<Arc<dyn CloudEventsSink>>) -> Self {
        Self { sinks }
    }
}

#[async_trait]
impl CloudEventsSink for MultiSink {
    async fn publish(&self, events: Vec<CloudEventEnvelope>) -> Result<(), SinkError> {
        // Publish to all sinks, collecting errors
        let mut first_error: Option<SinkError> = None;

        for sink in &self.sinks {
            if let Err(e) = sink.publish(events.clone()).await {
                tracing::error!(
                    sink = %sink.name(),
                    error = %e,
                    "CloudEvents sink failed"
                );
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }

        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn name(&self) -> &str {
        "multi"
    }
}

/// No-op sink for testing or when no sink is configured.
pub struct NullSink;

#[async_trait]
impl CloudEventsSink for NullSink {
    async fn publish(&self, events: Vec<CloudEventEnvelope>) -> Result<(), SinkError> {
        tracing::debug!(
            event_count = events.len(),
            "CloudEvents discarded (null sink)"
        );
        Ok(())
    }

    fn name(&self) -> &str {
        "null"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudevents::{EventBuilder, EventBuilderV10};
    use std::sync::atomic::{AtomicUsize, Ordering};

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
        async fn publish(&self, events: Vec<CloudEventEnvelope>) -> Result<(), SinkError> {
            self.count.fetch_add(events.len(), Ordering::SeqCst);
            Ok(())
        }

        fn name(&self) -> &str {
            "counting"
        }
    }

    #[tokio::test]
    async fn test_null_sink() {
        let sink = NullSink;
        let events = vec![test_event("test-1")];
        let result = sink.publish(events).await;
        assert!(result.is_ok());
    }

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

        let result = multi.publish(events).await;
        assert!(result.is_ok());
        assert_eq!(sink1.count(), 3);
        assert_eq!(sink2.count(), 3);
    }
}
