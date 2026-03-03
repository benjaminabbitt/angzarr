//! Tests for CloudEvents protobuf encoding.
//!
//! CloudEvents can be serialized to protobuf format (io.cloudevents.v1).
//! This enables efficient binary transmission for high-throughput sinks.
//!
//! Key behaviors verified:
//! - Single event encodes to CloudEvent message
//! - Batch encodes to CloudEventBatch message
//! - Data (JSON/text/binary) is preserved
//! - Extensions are encoded as attributes
//! - Empty batch is valid (empty repeated field)

use super::*;
use cloudevents::{EventBuilder, EventBuilderV10};

// ============================================================================
// Single Event Encoding Tests
// ============================================================================

/// Single event encodes to CloudEvent protobuf message.
#[test]
fn test_encode_single() {
    let event = EventBuilderV10::new()
        .id("single-001")
        .source("angzarr/test")
        .ty("test.SingleEvent")
        .build()
        .expect("valid event");

    let bytes = encode_proto_single(&event).unwrap();
    let proto_event = ProtoCloudEvent::decode(bytes.as_slice()).unwrap();

    assert_eq!(proto_event.id, "single-001");
    assert_eq!(proto_event.source, "angzarr/test");
    assert_eq!(proto_event.r#type, "test.SingleEvent");
    assert_eq!(proto_event.spec_version, "1.0");
}

// ============================================================================
// Batch Encoding Tests
// ============================================================================

/// Empty batch produces valid (empty) CloudEventBatch.
#[test]
fn test_encode_empty_batch() {
    let events: Vec<CloudEventEnvelope> = vec![];
    let bytes = encode_proto_batch(&events).unwrap();

    // Empty batch produces empty bytes in protobuf (no fields set)
    // This is valid - an empty repeated field is just absent
    let batch = CloudEventBatch::decode(bytes.as_slice()).unwrap();
    assert!(batch.events.is_empty());
}

/// Single event in batch preserves all required fields.
#[test]
fn test_encode_single_event() {
    let event = EventBuilderV10::new()
        .id("test-123")
        .source("angzarr/test")
        .ty("test.Event")
        .build()
        .expect("valid event");

    let bytes = encode_proto_batch(&[event]).unwrap();
    let batch = CloudEventBatch::decode(bytes.as_slice()).unwrap();

    assert_eq!(batch.events.len(), 1);
    let proto_event = &batch.events[0];
    assert_eq!(proto_event.id, "test-123");
    assert_eq!(proto_event.source, "angzarr/test");
    assert_eq!(proto_event.r#type, "test.Event");
    assert_eq!(proto_event.spec_version, "1.0");
}

/// JSON data is encoded as TextData in protobuf.
#[test]
fn test_encode_event_with_data() {
    let event = EventBuilderV10::new()
        .id("test-456")
        .source("angzarr/test")
        .ty("test.DataEvent")
        .data("application/json", serde_json::json!({"key": "value"}))
        .build()
        .expect("valid event");

    let bytes = encode_proto_batch(&[event]).unwrap();
    let batch = CloudEventBatch::decode(bytes.as_slice()).unwrap();

    let proto_event = &batch.events[0];
    assert!(proto_event.data.is_some());
    match &proto_event.data {
        Some(ProtoData::TextData(text)) => {
            assert!(text.contains("key"));
            assert!(text.contains("value"));
        }
        other => panic!("Expected TextData, got {:?}", other),
    }
}

/// Extensions are encoded as typed attributes.
#[test]
fn test_encode_event_with_extensions() {
    let event = EventBuilderV10::new()
        .id("test-789")
        .source("angzarr/test")
        .ty("test.ExtEvent")
        .extension("correlationid", "corr-abc")
        .extension("priority", 5i64)
        .build()
        .expect("valid event");

    let bytes = encode_proto_batch(&[event]).unwrap();
    let batch = CloudEventBatch::decode(bytes.as_slice()).unwrap();

    let proto_event = &batch.events[0];
    assert!(proto_event.attributes.contains_key("correlationid"));
    assert!(proto_event.attributes.contains_key("priority"));

    let corr = proto_event.attributes.get("correlationid").unwrap();
    match &corr.attr {
        Some(Attr::CeString(s)) => assert_eq!(s, "corr-abc"),
        other => panic!("Expected CeString, got {:?}", other),
    }

    let prio = proto_event.attributes.get("priority").unwrap();
    match &prio.attr {
        Some(Attr::CeInteger(i)) => assert_eq!(*i, 5),
        other => panic!("Expected CeInteger, got {:?}", other),
    }
}

/// Multiple events encode to batch with correct count.
#[test]
fn test_encode_multiple_events() {
    let events: Vec<CloudEventEnvelope> = (0..3)
        .map(|i| {
            EventBuilderV10::new()
                .id(format!("test-{}", i))
                .source("angzarr/test")
                .ty("test.BatchEvent")
                .build()
                .expect("valid event")
        })
        .collect();

    let bytes = encode_proto_batch(&events).unwrap();
    let batch = CloudEventBatch::decode(bytes.as_slice()).unwrap();

    assert_eq!(batch.events.len(), 3);
    for (i, proto_event) in batch.events.iter().enumerate() {
        assert_eq!(proto_event.id, format!("test-{}", i));
    }
}
