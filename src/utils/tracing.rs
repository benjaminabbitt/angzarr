//! Trace context propagation utilities.
//!
//! Provides helpers for injecting and extracting W3C TraceContext headers
//! across message bus boundaries. Each bus implementation provides its own
//! Injector/Extractor types while this module handles the common propagation logic.

/// Inject current span's trace context into a carrier.
///
/// Use this when publishing messages to inject trace context for distributed tracing.
///
/// # Example
/// ```ignore
/// struct MyInjector<'a>(&'a mut HashMap<String, String>);
/// impl Injector for MyInjector<'_> {
///     fn set(&mut self, key: &str, value: String) {
///         self.0.insert(key.to_string(), value);
///     }
/// }
///
/// let mut headers = HashMap::new();
/// inject_trace_context(&mut MyInjector(&mut headers));
/// ```
#[cfg(feature = "otel")]
pub fn inject_trace_context<I: opentelemetry::propagation::Injector>(injector: &mut I) {
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    let cx = tracing::Span::current().context();
    opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&cx, injector);
    });
}

/// Extract trace context from a carrier and set as parent on the given span.
///
/// Use this when consuming messages to link the handler span to the producer's trace.
///
/// # Example
/// ```ignore
/// struct MyExtractor<'a>(&'a HashMap<String, String>);
/// impl Extractor for MyExtractor<'_> {
///     fn get(&self, key: &str) -> Option<&str> {
///         self.0.get(key).map(|s| s.as_str())
///     }
///     fn keys(&self) -> Vec<&str> {
///         self.0.keys().map(|k| k.as_str()).collect()
///     }
/// }
///
/// let span = tracing::info_span!("handle_message");
/// extract_trace_context(&MyExtractor(&headers), &span);
/// ```
#[cfg(feature = "otel")]
pub fn extract_trace_context<E: opentelemetry::propagation::Extractor>(
    extractor: &E,
    span: &tracing::Span,
) {
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    let parent_cx =
        opentelemetry::global::get_text_map_propagator(|propagator| propagator.extract(extractor));
    let _ = span.set_parent(parent_cx);
}

/// No-op version when otel feature is disabled.
#[cfg(not(feature = "otel"))]
pub fn inject_trace_context<I>(_injector: &mut I) {}

/// No-op version when otel feature is disabled.
#[cfg(not(feature = "otel"))]
pub fn extract_trace_context<E>(_extractor: &E, _span: &tracing::Span) {}

#[cfg(test)]
#[path = "tracing.test.rs"]
mod tests;
