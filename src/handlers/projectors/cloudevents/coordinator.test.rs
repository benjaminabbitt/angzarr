//! Tests for CloudEvents coordinator.
//!
//! The coordinator receives Projections from projector handlers, detects
//! CloudEventsResponse in the projection field, converts to CloudEvents
//! JSON, and publishes to sinks. Tests verify:
//! - Event conversion with defaults and overrides
//! - Extension key normalization (lowercase per CloudEvents spec)
//! - Correlation ID propagation
//! - Base64 encoding for binary fallback

use super::*;
use crate::handlers::projectors::cloudevents::sink::NullSink;
use cloudevents::event::{AttributesReader, ExtensionValue};

fn create_test_coordinator() -> CloudEventsCoordinator {
    CloudEventsCoordinator::new(Arc::new(NullSink))
}

// ============================================================================
// Base64 Encoding Tests
// ============================================================================

/// Base64 encoding for binary data fallback.
///
/// When proto types aren't in the descriptor pool, we fall back to
/// base64-encoded binary representation.
#[test]
fn test_base64_encode() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

// ============================================================================
// Event Conversion Tests
// ============================================================================

/// Missing fields get sensible defaults.
///
/// CloudEvents without explicit id/source/subject get defaults derived
/// from the angzarr context (domain, root_id, sequence).
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

/// Explicit values override defaults.
///
/// When CloudEvent has explicit id/source/subject, those are used
/// instead of derived defaults.
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

// ============================================================================
// Extension Key Normalization Tests
// ============================================================================

/// Extension keys are lowercased per CloudEvents spec.
///
/// CloudEvents 1.0 spec requires extension names to be lowercase.
/// Mixed-case input is normalized to lowercase.
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
