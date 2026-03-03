//! CloudEvents types using the official SDK.
//!
//! Re-exports the cloudevents-sdk Event type and provides utilities
//! for building events from angzarr metadata.

pub use cloudevents::Event as CloudEventEnvelope;
use cloudevents::{EventBuilder, EventBuilderV10};

/// Content type for CloudEvents serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentType {
    /// JSON format (application/cloudevents+json).
    #[default]
    Json,
    /// Protobuf format (application/cloudevents+protobuf).
    Protobuf,
}

impl ContentType {
    /// Parse from string (for env vars, config).
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "protobuf" | "proto" | "pb" => Self::Protobuf,
            _ => Self::Json,
        }
    }

    /// MIME type for HTTP Content-Type header (batch format).
    pub fn batch_mime_type(&self) -> &'static str {
        match self {
            Self::Json => "application/cloudevents-batch+json",
            Self::Protobuf => "application/cloudevents-batch+protobuf",
        }
    }

    /// MIME type for single event.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Json => "application/cloudevents+json",
            Self::Protobuf => "application/cloudevents+protobuf",
        }
    }
}

/// Extension for building CloudEvents from angzarr metadata.
pub trait CloudEventBuilderExt {
    /// Create a new CloudEvents 1.0 builder with required fields.
    fn angzarr(
        id: impl Into<String>,
        event_type: impl Into<String>,
        source: impl Into<String>,
    ) -> EventBuilderV10;
}

impl CloudEventBuilderExt for EventBuilderV10 {
    fn angzarr(
        id: impl Into<String>,
        event_type: impl Into<String>,
        source: impl Into<String>,
    ) -> EventBuilderV10 {
        EventBuilderV10::new().id(id).ty(event_type).source(source)
    }
}

/// Normalize an extension key to lowercase per CloudEvents spec.
///
/// CloudEvents spec requires extension attribute names to be lowercase.
/// We accept any case from clients but always emit lowercase.
pub fn normalize_extension_key(key: &str) -> String {
    key.to_lowercase()
}

#[cfg(test)]
#[path = "types.test.rs"]
mod tests;
