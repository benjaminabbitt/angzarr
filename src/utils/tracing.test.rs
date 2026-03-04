//! Tests for trace context propagation utilities.
//!
//! These utilities enable distributed tracing across message bus boundaries
//! by injecting/extracting W3C TraceContext headers.
//!
//! Why this matters: Without trace context propagation, distributed traces
//! break at service boundaries. These functions ensure the `traceparent`
//! header flows through message buses (AMQP, Kafka, etc.).
//!
//! Tests are feature-gated: only compiled when `otel` feature is enabled.

use super::*;
use std::collections::HashMap;

// ============================================================================
// Mock Injector/Extractor for testing
// ============================================================================

/// Mock injector that collects headers into a HashMap.
struct MockInjector<'a>(&'a mut HashMap<String, String>);

#[cfg(feature = "otel")]
impl opentelemetry::propagation::Injector for MockInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

/// Mock extractor that reads headers from a HashMap.
struct MockExtractor<'a>(&'a HashMap<String, String>);

#[cfg(feature = "otel")]
impl opentelemetry::propagation::Extractor for MockExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|s| s.as_str())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

// ============================================================================
// inject_trace_context tests
// ============================================================================

/// inject_trace_context accepts any Injector implementation.
///
/// The function should compile and run without panicking for valid injectors.
#[test]
#[cfg(feature = "otel")]
fn test_inject_trace_context_accepts_injector() {
    let mut headers = HashMap::new();
    let mut injector = MockInjector(&mut headers);

    // Should not panic - may not inject anything without active span context
    inject_trace_context(&mut injector);
}

/// inject_trace_context with no active span doesn't inject traceparent.
///
/// When there's no active tracing span, no trace context should be propagated.
#[test]
#[cfg(feature = "otel")]
fn test_inject_with_no_span_no_traceparent() {
    let mut headers = HashMap::new();
    let mut injector = MockInjector(&mut headers);

    inject_trace_context(&mut injector);

    // Without an active OTel span, no traceparent header should be injected
    // This is expected behavior - the function is a no-op without context
    assert!(
        !headers.contains_key("traceparent")
            || headers.get("traceparent").map_or(true, |v| v.is_empty()),
        "Without active span, traceparent should not be present or should be empty"
    );
}

// ============================================================================
// extract_trace_context tests
// ============================================================================

/// extract_trace_context accepts any Extractor implementation.
///
/// The function should compile and run without panicking for valid extractors.
#[test]
#[cfg(feature = "otel")]
fn test_extract_trace_context_accepts_extractor() {
    let headers = HashMap::new();
    let extractor = MockExtractor(&headers);
    let span = tracing::info_span!("test_span");

    // Should not panic
    extract_trace_context(&extractor, &span);
}

/// extract_trace_context with valid traceparent doesn't panic.
///
/// Even with a properly formatted traceparent header, the function should
/// handle extraction gracefully.
#[test]
#[cfg(feature = "otel")]
fn test_extract_with_traceparent_header() {
    let mut headers = HashMap::new();
    // W3C traceparent format: version-trace_id-parent_id-flags
    headers.insert(
        "traceparent".to_string(),
        "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".to_string(),
    );

    let extractor = MockExtractor(&headers);
    let span = tracing::info_span!("test_span");

    // Should not panic - processes the traceparent header
    extract_trace_context(&extractor, &span);
}

/// extract_trace_context with empty headers doesn't panic.
///
/// When no trace context exists, the function should be a no-op.
#[test]
#[cfg(feature = "otel")]
fn test_extract_with_empty_headers() {
    let headers = HashMap::new();
    let extractor = MockExtractor(&headers);
    let span = tracing::info_span!("test_span");

    // Should not panic
    extract_trace_context(&extractor, &span);
}

/// extract_trace_context with malformed traceparent doesn't panic.
///
/// Invalid trace context should be ignored gracefully.
#[test]
#[cfg(feature = "otel")]
fn test_extract_with_malformed_traceparent() {
    let mut headers = HashMap::new();
    headers.insert("traceparent".to_string(), "invalid-format".to_string());

    let extractor = MockExtractor(&headers);
    let span = tracing::info_span!("test_span");

    // Should not panic - invalid traceparent is ignored
    extract_trace_context(&extractor, &span);
}

// ============================================================================
// No-op fallback tests (when otel feature is disabled)
// ============================================================================

/// inject_trace_context compiles without otel feature.
///
/// The no-op version should compile and do nothing.
#[test]
#[cfg(not(feature = "otel"))]
fn test_inject_noop_compiles() {
    let mut headers = HashMap::<String, String>::new();
    // Without otel, this is a no-op that accepts any type
    inject_trace_context(&mut headers);
}

/// extract_trace_context compiles without otel feature.
///
/// The no-op version should compile and do nothing.
#[test]
#[cfg(not(feature = "otel"))]
fn test_extract_noop_compiles() {
    let headers = HashMap::<String, String>::new();
    let span = tracing::info_span!("test_span");
    // Without otel, this is a no-op
    extract_trace_context(&headers, &span);
}
