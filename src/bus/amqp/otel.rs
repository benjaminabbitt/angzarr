//! OpenTelemetry trace context propagation for AMQP.
//!
//! Implements W3C TraceContext propagation via message headers.

use lapin::types::FieldTable;
use lapin::BasicProperties;

/// AMQP-specific injector for W3C trace context headers.
pub(super) struct AmqpInjector<'a>(
    pub &'a mut std::collections::BTreeMap<lapin::types::ShortString, lapin::types::AMQPValue>,
);

impl opentelemetry::propagation::Injector for AmqpInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        use lapin::types::AMQPValue;
        self.0
            .insert(key.into(), AMQPValue::LongString(value.into()));
    }
}

/// AMQP-specific extractor for W3C trace context headers.
pub(super) struct AmqpExtractor<'a>(pub &'a FieldTable);

impl opentelemetry::propagation::Extractor for AmqpExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        use lapin::types::AMQPValue;
        self.0.inner().get(key).and_then(|v| match v {
            AMQPValue::LongString(s) => std::str::from_utf8(s.as_bytes()).ok(),
            _ => None,
        })
    }
    fn keys(&self) -> Vec<&str> {
        self.0.inner().keys().map(|k| k.as_str()).collect()
    }
}

/// Inject W3C trace context from the current span into AMQP message headers.
pub(super) fn amqp_inject_trace_context() -> FieldTable {
    let mut headers = std::collections::BTreeMap::new();
    crate::utils::tracing::inject_trace_context(&mut AmqpInjector(&mut headers));
    FieldTable::from(headers)
}

/// Extract W3C trace context from AMQP message properties and set as parent on span.
pub(super) fn amqp_extract_trace_context(properties: &BasicProperties, span: &tracing::Span) {
    if let Some(headers) = properties.headers() {
        crate::utils::tracing::extract_trace_context(&AmqpExtractor(headers), span);
    }
}
