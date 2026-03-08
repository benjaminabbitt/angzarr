//! OpenTelemetry trace context propagation for Pub/Sub.
//!
//! Implements W3C TraceContext propagation via message attributes.

use std::collections::HashMap;

/// Pub/Sub-specific injector for W3C trace context into message attributes.
pub(super) struct PubSubInjector<'a>(pub &'a mut HashMap<String, String>);

impl opentelemetry::propagation::Injector for PubSubInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

/// Pub/Sub-specific extractor for W3C trace context from message attributes.
pub(super) struct PubSubExtractor<'a>(pub &'a HashMap<String, String>);

impl opentelemetry::propagation::Extractor for PubSubExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|s| s.as_str())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Inject W3C trace context from the current span into Pub/Sub message attributes.
pub(super) fn pubsub_inject_trace_context(attrs: &mut HashMap<String, String>) {
    crate::utils::tracing::inject_trace_context(&mut PubSubInjector(attrs));
}

/// Extract W3C trace context from Pub/Sub message attributes and set as parent on span.
pub(super) fn pubsub_extract_trace_context(attrs: &HashMap<String, String>, span: &tracing::Span) {
    crate::utils::tracing::extract_trace_context(&PubSubExtractor(attrs), span);
}
