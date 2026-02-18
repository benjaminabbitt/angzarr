//! CloudEvents coordinator.
//!
//! Receives Projections from projector handlers, detects CloudEventsResponse
//! in the projection field, converts to CloudEvents JSON, and publishes to sinks.

use std::sync::Arc;

use cloudevents::{EventBuilder, EventBuilderV10};
use prost::Message;
use tracing::{debug, error, warn};

use crate::proto::{CloudEvent, CloudEventsResponse, EventBook, Projection};
use crate::proto_reflect;

use super::sink::{CloudEventsSink, SinkError};
use super::types::{normalize_extension_key, CloudEventEnvelope, ContentType};

/// CloudEvents coordinator.
///
/// Processes projections and routes CloudEventsResponse to configured sinks.
pub struct CloudEventsCoordinator {
    sink: Arc<dyn CloudEventsSink>,
    content_type: ContentType,
}

impl CloudEventsCoordinator {
    /// Create a new coordinator with the given sink.
    pub fn new(sink: Arc<dyn CloudEventsSink>) -> Self {
        Self {
            sink,
            content_type: ContentType::default(),
        }
    }

    /// Create with specific content type.
    pub fn with_content_type(mut self, content_type: ContentType) -> Self {
        self.content_type = content_type;
        self
    }

    /// Process a projection, publishing CloudEvents if applicable.
    ///
    /// Returns true if the projection was a CloudEventsResponse and was processed.
    pub async fn process(
        &self,
        projection: &Projection,
        source_events: Option<&EventBook>,
    ) -> Result<bool, SinkError> {
        // Check if projection contains CloudEventsResponse
        let Some(projection_any) = &projection.projection else {
            return Ok(false);
        };

        // Check type_url for CloudEventsResponse
        if !projection_any.type_url.ends_with("CloudEventsResponse") {
            return Ok(false);
        }

        // Decode CloudEventsResponse
        let response = match CloudEventsResponse::decode(&projection_any.value[..]) {
            Ok(r) => r,
            Err(e) => {
                error!(
                    projector = %projection.projector,
                    error = %e,
                    "Failed to decode CloudEventsResponse"
                );
                return Err(SinkError::Serialization(serde_json::Error::io(
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
                )));
            }
        };

        if response.events.is_empty() {
            debug!(
                projector = %projection.projector,
                "CloudEventsResponse empty, skipping"
            );
            return Ok(true);
        }

        // Convert to CloudEvents SDK events
        let envelopes = self.convert_events(&response.events, projection, source_events)?;

        // Publish to sink
        self.sink.publish(envelopes, self.content_type).await?;

        debug!(
            projector = %projection.projector,
            event_count = response.events.len(),
            sink = %self.sink.name(),
            "CloudEvents published"
        );

        Ok(true)
    }

    /// Convert proto CloudEvents to SDK Event objects.
    fn convert_events(
        &self,
        events: &[CloudEvent],
        projection: &Projection,
        source_events: Option<&EventBook>,
    ) -> Result<Vec<CloudEventEnvelope>, SinkError> {
        let cover = projection.cover.as_ref();
        let domain = cover.map(|c| c.domain.as_str()).unwrap_or("unknown");
        let root_id = cover
            .and_then(|c| c.root.as_ref())
            .map(|u| hex::encode(&u.value))
            .unwrap_or_else(|| "unknown".to_string());
        let correlation_id = cover.map(|c| c.correlation_id.as_str()).unwrap_or("");

        // Get timestamp from first source event if available
        let default_time = source_events
            .and_then(|e| e.pages.first())
            .and_then(|p| p.created_at.as_ref())
            .and_then(|ts| chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32))
            .unwrap_or_else(chrono::Utc::now);

        let mut envelopes = Vec::with_capacity(events.len());

        for (idx, event) in events.iter().enumerate() {
            let envelope = self.convert_single_event(
                event,
                domain,
                &root_id,
                correlation_id,
                default_time,
                projection.sequence.saturating_add(idx as u32),
            )?;
            envelopes.push(envelope);
        }

        Ok(envelopes)
    }

    /// Convert a single proto CloudEvent to SDK Event.
    fn convert_single_event(
        &self,
        event: &CloudEvent,
        domain: &str,
        root_id: &str,
        correlation_id: &str,
        default_time: chrono::DateTime<chrono::Utc>,
        sequence: u32,
    ) -> Result<CloudEventEnvelope, SinkError> {
        // Use provided values or defaults
        let id = event
            .id
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{}:{}:{}", domain, root_id, sequence));

        let event_type = if event.r#type.is_empty() {
            // Derive from data type_url if available
            event
                .data
                .as_ref()
                .map(|d| {
                    d.type_url
                        .rsplit('/')
                        .next()
                        .unwrap_or(&d.type_url)
                        .to_string()
                })
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            event.r#type.clone()
        };

        let source = event
            .source
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("angzarr/{}", domain));

        let subject = event
            .subject
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| root_id.to_string());

        // Convert data Any to JSON
        let data = event.data.as_ref().and_then(|any| self.any_to_json(any));

        // Build event using SDK builder
        let mut builder = EventBuilderV10::new()
            .id(id)
            .ty(event_type)
            .source(source)
            .time(default_time)
            .subject(subject);

        // Set data if present
        if let Some(json_data) = data {
            builder = builder.data("application/json", json_data);
        }

        // Add correlation_id as extension if present (lowercase per spec)
        if !correlation_id.is_empty() {
            builder = builder.extension("correlationid", correlation_id);
        }

        // Add client-provided extensions (normalize keys to lowercase)
        for (key, value) in &event.extensions {
            let normalized_key = normalize_extension_key(key);
            builder = builder.extension(&normalized_key, value.as_str());
        }

        // Build and validate the event
        builder.build().map_err(|e| {
            error!(error = %e, "Failed to build CloudEvent");
            SinkError::Config(format!("Invalid CloudEvent: {}", e))
        })
    }

    /// Convert proto Any to JSON Value using prost-reflect.
    fn any_to_json(&self, any: &prost_types::Any) -> Option<serde_json::Value> {
        // Try to decode using global descriptor pool
        match proto_reflect::decode_any(any) {
            Ok(msg) => {
                match serde_json::to_value(&msg) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!(
                            type_url = %any.type_url,
                            error = %e,
                            "Failed to serialize DynamicMessage to JSON"
                        );
                        // Fallback: base64 encode the binary
                        Some(serde_json::json!({
                            "_type": any.type_url,
                            "_binary": base64_encode(&any.value),
                            "_size": any.value.len()
                        }))
                    }
                }
            }
            Err(e) => {
                // Descriptor pool may not have this type
                debug!(
                    type_url = %any.type_url,
                    error = %e,
                    "Proto type not in descriptor pool, using binary fallback"
                );
                // Fallback: base64 encode the binary
                Some(serde_json::json!({
                    "_type": any.type_url,
                    "_binary": base64_encode(&any.value),
                    "_size": any.value.len()
                }))
            }
        }
    }
}

/// Base64 encode bytes (standard alphabet).
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        result.push(ALPHABET[b0 >> 2] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::projectors::cloudevents::sink::NullSink;
    use cloudevents::event::{AttributesReader, ExtensionValue};

    fn create_test_coordinator() -> CloudEventsCoordinator {
        CloudEventsCoordinator::new(Arc::new(NullSink))
    }

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_convert_single_event_defaults() {
        let coordinator = create_test_coordinator();

        let event = CloudEvent {
            r#type: "orders.OrderCreated".to_string(),
            data: None,
            extensions: Default::default(),
            id: None,
            source: None,
            subject: None,
        };

        let envelope = coordinator
            .convert_single_event(
                &event,
                "orders",
                "abc123",
                "corr-xyz",
                chrono::Utc::now(),
                5,
            )
            .unwrap();

        assert_eq!(envelope.id(), "orders:abc123:5");
        assert_eq!(envelope.ty(), "orders.OrderCreated");
        assert_eq!(envelope.source().to_string(), "angzarr/orders");
        assert_eq!(envelope.subject(), Some("abc123"));
        assert_eq!(
            envelope.extension("correlationid"),
            Some(&ExtensionValue::String("corr-xyz".to_string()))
        );
    }

    #[test]
    fn test_convert_single_event_overrides() {
        let coordinator = create_test_coordinator();

        let event = CloudEvent {
            r#type: "custom.Type".to_string(),
            data: None,
            extensions: [("myext".to_string(), "value".to_string())]
                .into_iter()
                .collect(),
            id: Some("custom-id".to_string()),
            source: Some("custom-source".to_string()),
            subject: Some("custom-subject".to_string()),
        };

        let envelope = coordinator
            .convert_single_event(&event, "orders", "abc123", "", chrono::Utc::now(), 0)
            .unwrap();

        assert_eq!(envelope.id(), "custom-id");
        assert_eq!(envelope.ty(), "custom.Type");
        assert_eq!(envelope.source().to_string(), "custom-source");
        assert_eq!(envelope.subject(), Some("custom-subject"));
        assert_eq!(
            envelope.extension("myext"),
            Some(&ExtensionValue::String("value".to_string()))
        );
        // No correlationid when empty
        assert!(envelope.extension("correlationid").is_none());
    }

    #[test]
    fn test_extension_keys_are_lowercased() {
        let coordinator = create_test_coordinator();

        let event = CloudEvent {
            r#type: "test.Event".to_string(),
            data: None,
            extensions: [
                ("MyCustomExt".to_string(), "value1".to_string()),
                ("UPPERCASE".to_string(), "value2".to_string()),
                ("MixedCase".to_string(), "value3".to_string()),
            ]
            .into_iter()
            .collect(),
            id: None,
            source: None,
            subject: None,
        };

        let envelope = coordinator
            .convert_single_event(&event, "test", "root", "", chrono::Utc::now(), 0)
            .unwrap();

        // All extensions should be lowercase
        assert_eq!(
            envelope.extension("mycustomext"),
            Some(&ExtensionValue::String("value1".to_string()))
        );
        assert_eq!(
            envelope.extension("uppercase"),
            Some(&ExtensionValue::String("value2".to_string()))
        );
        assert_eq!(
            envelope.extension("mixedcase"),
            Some(&ExtensionValue::String("value3".to_string()))
        );

        // Original case should not exist
        assert!(envelope.extension("MyCustomExt").is_none());
        assert!(envelope.extension("UPPERCASE").is_none());
        assert!(envelope.extension("MixedCase").is_none());
    }
}
