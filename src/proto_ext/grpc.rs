//! gRPC utilities for correlation ID and trace context propagation.

use angzarr_client::proto_ext::constants::CORRELATION_ID_HEADER;

/// Create a tonic Request with `x-correlation-id` gRPC metadata.
///
/// Propagates the correlation_id into gRPC request headers so that
/// server-side tower middleware can create tracing spans before
/// protobuf deserialization.
///
/// When the `otel` feature is enabled, also injects W3C `traceparent`
/// header for distributed trace context propagation.
pub fn correlated_request<T>(msg: T, correlation_id: &str) -> tonic::Request<T> {
    let mut req = tonic::Request::new(msg);
    if !correlation_id.is_empty() {
        if let Ok(val) = correlation_id.parse() {
            req.metadata_mut().insert(CORRELATION_ID_HEADER, val);
        }
    }

    #[cfg(feature = "otel")]
    {
        inject_trace_context(req.metadata_mut());
    }

    req
}

/// Inject W3C trace context into tonic metadata from the current tracing span.
#[cfg(feature = "otel")]
fn inject_trace_context(metadata: &mut tonic::metadata::MetadataMap) {
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    let cx = tracing::Span::current().context();

    opentelemetry::global::get_text_map_propagator(|propagator| {
        let mut injector = MetadataInjector(metadata);
        propagator.inject_context(&cx, &mut injector);
    });
}

/// Adapter to inject OTel context into tonic gRPC metadata.
#[cfg(feature = "otel")]
struct MetadataInjector<'a>(&'a mut tonic::metadata::MetadataMap);

#[cfg(feature = "otel")]
impl opentelemetry::propagation::Injector for MetadataInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(key) = tonic::metadata::MetadataKey::from_bytes(key.as_bytes()) {
            if let Ok(val) = value.parse() {
                self.0.insert(key, val);
            }
        }
    }
}
