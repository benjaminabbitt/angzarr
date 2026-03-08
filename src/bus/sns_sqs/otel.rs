//! OpenTelemetry trace context propagation for SNS/SQS.
//!
//! Implements W3C TraceContext propagation via message attributes.

use std::collections::HashMap;

/// SNS-specific injector for W3C trace context into message attributes.
pub(crate) struct SnsInjector<'a>(
    pub &'a mut HashMap<String, aws_sdk_sns::types::MessageAttributeValue>,
);

impl opentelemetry::propagation::Injector for SnsInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(attr) = aws_sdk_sns::types::MessageAttributeValue::builder()
            .data_type("String")
            .string_value(value)
            .build()
        {
            self.0.insert(key.to_string(), attr);
        }
    }
}

/// SQS-specific extractor for W3C trace context from message attributes.
pub(crate) struct SqsExtractor<'a>(
    pub &'a HashMap<String, aws_sdk_sqs::types::MessageAttributeValue>,
);

impl opentelemetry::propagation::Extractor for SqsExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.string_value())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Inject W3C trace context from the current span into SNS message attributes.
pub(crate) fn sns_inject_trace_context(
    attrs: &mut HashMap<String, aws_sdk_sns::types::MessageAttributeValue>,
) {
    crate::utils::tracing::inject_trace_context(&mut SnsInjector(attrs));
}

/// Extract W3C trace context from SQS message attributes and set as parent on span.
pub(crate) fn sqs_extract_trace_context(
    message: &aws_sdk_sqs::types::Message,
    span: &tracing::Span,
) {
    if let Some(attrs) = message.message_attributes() {
        crate::utils::tracing::extract_trace_context(&SqsExtractor(attrs), span);
    }
}
