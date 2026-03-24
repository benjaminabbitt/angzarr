//! gRPC trace layer for request tracing.

use tower_http::trace::TraceLayer;

/// Tower trace layer that extracts `x-correlation-id` from gRPC request headers.
///
/// Creates a tracing span per request with the correlation_id, enabling
/// all downstream tracing to inherit it automatically. This works at the HTTP
/// layer — before tonic deserializes the protobuf body.
///
/// When the `otel` feature is enabled, also extracts W3C `traceparent` header
/// and sets it as the parent context on the span for distributed tracing.
pub fn grpc_trace_layer() -> TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::GrpcErrorsAsFailures>,
    impl Fn(&http::Request<tonic::body::Body>) -> tracing::Span + Clone,
> {
    TraceLayer::new_for_grpc().make_span_with(|request: &http::Request<tonic::body::Body>| {
        let correlation_id = request
            .headers()
            .get(crate::proto_ext::CORRELATION_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let path = request.uri().path();
        let span = tracing::info_span!("grpc", %correlation_id, %path);

        #[cfg(feature = "otel")]
        {
            extract_trace_context(request.headers(), &span);
        }

        span
    })
}

/// Extract W3C trace context from HTTP headers and set as parent on the span.
#[cfg(feature = "otel")]
fn extract_trace_context(headers: &http::HeaderMap, span: &tracing::Span) {
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    let parent_cx = opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(headers))
    });
    let _ = span.set_parent(parent_cx);
}

/// Adapter to extract OTel context from HTTP headers.
#[cfg(feature = "otel")]
struct HeaderExtractor<'a>(&'a http::HeaderMap);

#[cfg(feature = "otel")]
impl opentelemetry::propagation::Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}
