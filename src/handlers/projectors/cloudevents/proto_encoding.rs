//! Protobuf encoding for CloudEvents.
//!
//! Converts `cloudevents::Event` instances to the official CloudEvents
//! protobuf wire format (io.cloudevents.v1.CloudEventBatch).

use prost::Message;

use super::sink::SinkError;
use super::types::CloudEventEnvelope;

/// Generated CloudEvents protobuf types from io.cloudevents.v1 spec.
#[allow(clippy::enum_variant_names)]
pub mod io_cloudevents_v1 {
    tonic::include_proto!("io.cloudevents.v1");
}

use io_cloudevents_v1::{
    cloud_event::Data as ProtoData, cloud_event_attribute_value::Attr,
    CloudEvent as ProtoCloudEvent, CloudEventAttributeValue, CloudEventBatch,
};

/// Convert a single CloudEvent to the protobuf message type.
fn to_proto_event(event: &CloudEventEnvelope) -> ProtoCloudEvent {
    use cloudevents::event::{AttributesReader, ExtensionValue};

    // Convert extensions to attributes map
    let mut attributes = std::collections::HashMap::new();

    // Add time if present
    if let Some(time) = event.time() {
        attributes.insert(
            "time".to_string(),
            CloudEventAttributeValue {
                attr: Some(Attr::CeTimestamp(prost_types::Timestamp {
                    seconds: time.timestamp(),
                    nanos: time.timestamp_subsec_nanos() as i32,
                })),
            },
        );
    }

    // Add subject if present
    if let Some(subject) = event.subject() {
        attributes.insert(
            "subject".to_string(),
            CloudEventAttributeValue {
                attr: Some(Attr::CeString(subject.to_string())),
            },
        );
    }

    // Add datacontenttype if present
    if let Some(content_type) = event.datacontenttype() {
        attributes.insert(
            "datacontenttype".to_string(),
            CloudEventAttributeValue {
                attr: Some(Attr::CeString(content_type.to_string())),
            },
        );
    }

    // Add custom extensions
    for (key, value) in event.iter_extensions() {
        let attr_value = match value {
            ExtensionValue::String(s) => Attr::CeString(s.clone()),
            ExtensionValue::Integer(i) => Attr::CeInteger(*i as i32),
            ExtensionValue::Boolean(b) => Attr::CeBoolean(*b),
        };
        attributes.insert(
            key.to_string(),
            CloudEventAttributeValue {
                attr: Some(attr_value),
            },
        );
    }

    // Convert data
    let data = event.data().map(|d| match d {
        cloudevents::Data::Binary(bytes) => ProtoData::BinaryData(bytes.clone()),
        cloudevents::Data::String(s) => ProtoData::TextData(s.clone()),
        cloudevents::Data::Json(json) => {
            // JSON data is encoded as text
            ProtoData::TextData(json.to_string())
        }
    });

    ProtoCloudEvent {
        id: event.id().to_string(),
        source: event.source().to_string(),
        spec_version: event.specversion().to_string(),
        r#type: event.ty().to_string(),
        attributes,
        data,
    }
}

/// Encode a single CloudEvent to protobuf wire format.
///
/// Converts a `cloudevents::Event` to `io.cloudevents.v1.CloudEvent`
/// and serializes to bytes.
#[cfg(any(feature = "kafka", test))]
pub fn encode_proto_single(event: &CloudEventEnvelope) -> Result<Vec<u8>, SinkError> {
    let proto_event = to_proto_event(event);
    Ok(proto_event.encode_to_vec())
}

/// Encode a batch of CloudEvents to protobuf wire format.
///
/// Converts `cloudevents::Event` instances to `io.cloudevents.v1.CloudEventBatch`
/// and serializes to bytes.
pub fn encode_proto_batch(events: &[CloudEventEnvelope]) -> Result<Vec<u8>, SinkError> {
    let proto_events: Vec<ProtoCloudEvent> = events.iter().map(to_proto_event).collect();

    let batch = CloudEventBatch {
        events: proto_events,
    };

    Ok(batch.encode_to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloudevents::{EventBuilder, EventBuilderV10};

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

    #[test]
    fn test_encode_empty_batch() {
        let events: Vec<CloudEventEnvelope> = vec![];
        let bytes = encode_proto_batch(&events).unwrap();

        // Empty batch produces empty bytes in protobuf (no fields set)
        // This is valid - an empty repeated field is just absent
        let batch = CloudEventBatch::decode(bytes.as_slice()).unwrap();
        assert!(batch.events.is_empty());
    }

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
}
