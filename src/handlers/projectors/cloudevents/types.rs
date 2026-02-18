//! CloudEvents types using the official SDK.
//!
//! Re-exports the cloudevents-sdk Event type and provides utilities
//! for building events from angzarr metadata.

pub use cloudevents::Event as CloudEventEnvelope;
use cloudevents::{EventBuilder, EventBuilderV10};

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
mod tests {
    use super::*;
    use cloudevents::event::AttributesReader;
    use cloudevents::EventBuilder;

    #[test]
    fn test_build_minimal_event() {
        let event =
            EventBuilderV10::angzarr("orders:abc:1", "orders.OrderCreated", "angzarr/orders")
                .build()
                .expect("should build valid event");

        assert_eq!(event.specversion(), cloudevents::event::SpecVersion::V10);
        assert_eq!(event.id(), "orders:abc:1");
        assert_eq!(event.ty(), "orders.OrderCreated");
        assert_eq!(event.source().to_string(), "angzarr/orders");
    }

    #[test]
    fn test_build_event_with_data() {
        let event =
            EventBuilderV10::angzarr("orders:abc:1", "orders.OrderCreated", "angzarr/orders")
                .time(chrono::Utc::now())
                .subject("abc")
                .data("application/json", serde_json::json!({"order_id": "123"}))
                .build()
                .expect("should build valid event");

        assert!(event.time().is_some());
        assert_eq!(event.subject(), Some("abc"));
        assert!(event.data().is_some());
    }

    #[test]
    fn test_build_event_with_extension() {
        use cloudevents::event::ExtensionValue;

        let event =
            EventBuilderV10::angzarr("orders:abc:1", "orders.OrderCreated", "angzarr/orders")
                .extension("correlationid", "corr-xyz")
                .build()
                .expect("should build valid event");

        assert_eq!(
            event.extension("correlationid"),
            Some(&ExtensionValue::String("corr-xyz".to_string()))
        );
    }

    #[test]
    fn test_normalize_extension_key() {
        assert_eq!(normalize_extension_key("CorrelationID"), "correlationid");
        assert_eq!(normalize_extension_key("PRIORITY"), "priority");
        assert_eq!(normalize_extension_key("customext"), "customext");
        assert_eq!(normalize_extension_key("MyCustomExt"), "mycustomext");
    }

    #[test]
    fn test_event_serializes_to_json() {
        let event = EventBuilderV10::angzarr("test:123:0", "test.Event", "angzarr/test")
            .extension("customext", "value")
            .build()
            .expect("should build valid event");

        let json = serde_json::to_string(&event).expect("should serialize");
        assert!(json.contains(r#""specversion":"1.0""#));
        assert!(json.contains(r#""id":"test:123:0""#));
        assert!(json.contains(r#""type":"test.Event""#));
        assert!(json.contains(r#""customext":"value""#));
    }
}
