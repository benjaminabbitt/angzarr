//! Tests for CloudEvents types and builders.
//!
//! CloudEvents is a specification for describing event data in a common way.
//! This module provides utilities for building CloudEvents from angzarr metadata.
//!
//! Key behaviors verified:
//! - EventBuilderV10::angzarr() creates valid CloudEvents 1.0
//! - Events can include data, subject, time, and extensions
//! - Extension keys are normalized to lowercase per spec
//! - Events serialize to valid JSON

use super::*;
use cloudevents::event::AttributesReader;
use cloudevents::EventBuilder;

// ============================================================================
// Event Builder Tests
// ============================================================================

/// Minimal event has required fields: id, type, source.
#[test]
fn test_build_minimal_event() {
    let event = EventBuilderV10::angzarr("orders:abc:1", "orders.OrderCreated", "angzarr/orders")
        .build()
        .expect("should build valid event");

    assert_eq!(event.specversion(), cloudevents::event::SpecVersion::V10);
    assert_eq!(event.id(), "orders:abc:1");
    assert_eq!(event.ty(), "orders.OrderCreated");
    assert_eq!(event.source().to_string(), "angzarr/orders");
}

/// Events can include optional time, subject, and data.
#[test]
fn test_build_event_with_data() {
    let event = EventBuilderV10::angzarr("orders:abc:1", "orders.OrderCreated", "angzarr/orders")
        .time(chrono::Utc::now())
        .subject("abc")
        .data("application/json", serde_json::json!({"order_id": "123"}))
        .build()
        .expect("should build valid event");

    assert!(event.time().is_some());
    assert_eq!(event.subject(), Some("abc"));
    assert!(event.data().is_some());
}

/// Events can include custom extensions.
#[test]
fn test_build_event_with_extension() {
    use cloudevents::event::ExtensionValue;

    let event = EventBuilderV10::angzarr("orders:abc:1", "orders.OrderCreated", "angzarr/orders")
        .extension("correlationid", "corr-xyz")
        .build()
        .expect("should build valid event");

    assert_eq!(
        event.extension("correlationid"),
        Some(&ExtensionValue::String("corr-xyz".to_string()))
    );
}

// ============================================================================
// Extension Key Normalization Tests
// ============================================================================

/// Extension keys are normalized to lowercase per CloudEvents spec.
#[test]
fn test_normalize_extension_key() {
    assert_eq!(normalize_extension_key("CorrelationID"), "correlationid");
    assert_eq!(normalize_extension_key("PRIORITY"), "priority");
    assert_eq!(normalize_extension_key("customext"), "customext");
    assert_eq!(normalize_extension_key("MyCustomExt"), "mycustomext");
}

// ============================================================================
// Serialization Tests
// ============================================================================

/// Events serialize to valid CloudEvents JSON.
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
