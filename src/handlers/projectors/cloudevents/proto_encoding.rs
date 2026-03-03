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
#[path = "proto_encoding.test.rs"]
mod tests;
