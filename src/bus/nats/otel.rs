//! OpenTelemetry trace context propagation for NATS.
//!
//! Implements W3C TraceContext propagation via message headers.

use async_nats::HeaderMap;

/// NATS-specific injector for W3C trace context headers.
pub(super) struct NatsInjector<'a>(pub &'a mut HeaderMap);

impl opentelemetry::propagation::Injector for NatsInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key, value.as_str());
    }
}

/// NATS-specific extractor for W3C trace context headers.
pub(super) struct NatsExtractor<'a>(pub &'a HeaderMap);

impl opentelemetry::propagation::Extractor for NatsExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|v| v.as_str())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.iter().map(|(k, _)| k.as_ref()).collect()
    }
}

/// Inject W3C trace context from the current span into NATS message headers.
pub(super) fn nats_inject_trace_context(headers: &mut HeaderMap) {
    crate::utils::tracing::inject_trace_context(&mut NatsInjector(headers));
}

/// Extract W3C trace context from NATS message headers and set as parent on span.
pub(super) fn nats_extract_trace_context(headers: &HeaderMap, span: &tracing::Span) {
    crate::utils::tracing::extract_trace_context(&NatsExtractor(headers), span);
}
