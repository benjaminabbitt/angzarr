//! CloudEvents coordinator integration tests.
//!
//! Tests the flow from Projection → CloudEventsCoordinator → Sink.

use crate::common::*;
use angzarr::handlers::projectors::{
    CloudEventEnvelope, CloudEventsCoordinator, CloudEventsSink, ContentType, SinkError,
};
use angzarr::proto::{CloudEvent, CloudEventsResponse, Projection};
use cloudevents::event::{AttributesReader, ExtensionValue};
use prost::Message;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Mutex;

/// Recording sink that captures published CloudEvents for verification.
struct RecordingSink {
    events: Arc<Mutex<Vec<CloudEventEnvelope>>>,
    publish_count: AtomicUsize,
}

impl RecordingSink {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            publish_count: AtomicUsize::new(0),
        }
    }

    async fn get_events(&self) -> Vec<CloudEventEnvelope> {
        self.events.lock().await.clone()
    }

    fn publish_count(&self) -> usize {
        self.publish_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl CloudEventsSink for RecordingSink {
    async fn publish(
        &self,
        events: Vec<CloudEventEnvelope>,
        _format: ContentType,
    ) -> Result<(), SinkError> {
        self.publish_count.fetch_add(1, Ordering::SeqCst);
        let mut stored = self.events.lock().await;
        stored.extend(events);
        Ok(())
    }

    fn name(&self) -> &str {
        "recording"
    }
}

fn create_cloudevents_projection(domain: &str, root: Uuid, events: Vec<CloudEvent>) -> Projection {
    let response = CloudEventsResponse { events };
    let response_bytes = response.encode_to_vec();

    Projection {
        projector: "test-cloudevents-projector".to_string(),
        cover: Some(angzarr::proto::Cover {
            domain: domain.to_string(),
            root: Some(angzarr::proto::Uuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: "corr-123".to_string(),
            edition: None,
        }),
        projection: Some(Any {
            type_url: "type.googleapis.com/angzarr.CloudEventsResponse".to_string(),
            value: response_bytes,
        }),
        sequence: 1,
    }
}

fn create_regular_projection(domain: &str, root: Uuid) -> Projection {
    Projection {
        projector: "regular-projector".to_string(),
        cover: Some(angzarr::proto::Cover {
            domain: domain.to_string(),
            root: Some(angzarr::proto::Uuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: "corr-456".to_string(),
            edition: None,
        }),
        projection: Some(Any {
            type_url: "type.googleapis.com/some.OtherType".to_string(),
            value: vec![1, 2, 3],
        }),
        sequence: 1,
    }
}

#[tokio::test]
async fn test_coordinator_detects_cloudevents_response() {
    let sink = Arc::new(RecordingSink::new());
    let coordinator = CloudEventsCoordinator::new(sink.clone());

    let cloud_event = CloudEvent {
        r#type: "com.example.order.created".to_string(),
        data: None,
        extensions: Default::default(),
        id: None,
        source: None,
        subject: None,
    };

    let projection = create_cloudevents_projection("orders", Uuid::new_v4(), vec![cloud_event]);

    let result = coordinator.process(&projection, None).await;
    assert!(result.is_ok());
    assert!(result.unwrap(), "Should detect CloudEventsResponse");
    assert_eq!(sink.publish_count(), 1, "Should publish once");
}

#[tokio::test]
async fn test_coordinator_ignores_non_cloudevents_projection() {
    let sink = Arc::new(RecordingSink::new());
    let coordinator = CloudEventsCoordinator::new(sink.clone());

    let projection = create_regular_projection("orders", Uuid::new_v4());

    let result = coordinator.process(&projection, None).await;
    assert!(result.is_ok());
    assert!(!result.unwrap(), "Should ignore non-CloudEventsResponse");
    assert_eq!(sink.publish_count(), 0, "Should not publish");
}

#[tokio::test]
async fn test_coordinator_skips_empty_events() {
    let sink = Arc::new(RecordingSink::new());
    let coordinator = CloudEventsCoordinator::new(sink.clone());

    let projection = create_cloudevents_projection("orders", Uuid::new_v4(), vec![]);

    let result = coordinator.process(&projection, None).await;
    assert!(result.is_ok());
    assert!(result.unwrap(), "Should detect empty CloudEventsResponse");
    assert_eq!(sink.publish_count(), 0, "Should not publish empty batch");
}

#[tokio::test]
async fn test_coordinator_converts_event_fields() {
    let sink = Arc::new(RecordingSink::new());
    let coordinator = CloudEventsCoordinator::new(sink.clone());

    let root = Uuid::new_v4();

    let cloud_event = CloudEvent {
        r#type: "com.example.order.created".to_string(),
        data: None,
        extensions: [("priority".to_string(), "high".to_string())]
            .into_iter()
            .collect(),
        id: Some("custom-id-123".to_string()),
        source: Some("custom-source".to_string()),
        subject: Some("custom-subject".to_string()),
    };

    let projection = create_cloudevents_projection("orders", root, vec![cloud_event]);

    let result = coordinator.process(&projection, None).await;
    assert!(result.is_ok());

    let events = sink.get_events().await;
    assert_eq!(events.len(), 1);

    let envelope = &events[0];
    assert_eq!(envelope.id(), "custom-id-123");
    assert_eq!(envelope.ty(), "com.example.order.created");
    assert_eq!(envelope.source().to_string(), "custom-source");
    assert_eq!(envelope.subject(), Some("custom-subject"));
    assert_eq!(
        envelope.specversion(),
        cloudevents::event::SpecVersion::V10
    );
    assert_eq!(
        envelope.extension("priority"),
        Some(&ExtensionValue::String("high".to_string()))
    );
}

#[tokio::test]
async fn test_coordinator_uses_defaults_when_not_overridden() {
    let sink = Arc::new(RecordingSink::new());
    let coordinator = CloudEventsCoordinator::new(sink.clone());

    let root = Uuid::new_v4();

    let cloud_event = CloudEvent {
        r#type: "order.created".to_string(),
        data: None,
        extensions: Default::default(),
        id: None,
        source: None,
        subject: None,
    };

    let projection = create_cloudevents_projection("orders", root, vec![cloud_event]);

    let result = coordinator.process(&projection, None).await;
    assert!(result.is_ok());

    let events = sink.get_events().await;
    assert_eq!(events.len(), 1);

    let envelope = &events[0];
    let root_hex = hex::encode(root.as_bytes());

    // Default id: {domain}:{root_id}:{sequence}
    assert_eq!(envelope.id(), format!("orders:{}:1", root_hex));
    // Default source: angzarr/{domain}
    assert_eq!(envelope.source().to_string(), "angzarr/orders");
    // Default subject: aggregate root ID
    assert_eq!(envelope.subject(), Some(root_hex.as_str()));
    // Correlation ID added as extension
    assert_eq!(
        envelope.extension("correlationid"),
        Some(&ExtensionValue::String("corr-123".to_string()))
    );
}

#[tokio::test]
async fn test_coordinator_handles_multiple_events() {
    let sink = Arc::new(RecordingSink::new());
    let coordinator = CloudEventsCoordinator::new(sink.clone());

    let events = vec![
        CloudEvent {
            r#type: "order.created".to_string(),
            data: None,
            extensions: Default::default(),
            id: None,
            source: None,
            subject: None,
        },
        CloudEvent {
            r#type: "order.paid".to_string(),
            data: None,
            extensions: Default::default(),
            id: None,
            source: None,
            subject: None,
        },
        CloudEvent {
            r#type: "order.shipped".to_string(),
            data: None,
            extensions: Default::default(),
            id: None,
            source: None,
            subject: None,
        },
    ];

    let projection = create_cloudevents_projection("orders", Uuid::new_v4(), events);

    let result = coordinator.process(&projection, None).await;
    assert!(result.is_ok());

    let published = sink.get_events().await;
    assert_eq!(published.len(), 3);
    assert_eq!(published[0].ty(), "order.created");
    assert_eq!(published[1].ty(), "order.paid");
    assert_eq!(published[2].ty(), "order.shipped");
}

#[tokio::test]
async fn test_coordinator_handles_empty_projection_field() {
    let sink = Arc::new(RecordingSink::new());
    let coordinator = CloudEventsCoordinator::new(sink.clone());

    let projection = Projection {
        projector: "empty-projector".to_string(),
        cover: None,
        projection: None, // Empty projection field
        sequence: 0,
    };

    let result = coordinator.process(&projection, None).await;
    assert!(result.is_ok());
    assert!(!result.unwrap(), "Should return false for empty projection");
    assert_eq!(sink.publish_count(), 0);
}

/// Failing sink for error handling tests.
struct FailingSink {
    error_message: String,
}

impl FailingSink {
    fn new(message: &str) -> Self {
        Self {
            error_message: message.to_string(),
        }
    }
}

#[async_trait]
impl CloudEventsSink for FailingSink {
    async fn publish(
        &self,
        _events: Vec<CloudEventEnvelope>,
        _format: ContentType,
    ) -> Result<(), SinkError> {
        Err(SinkError::Unavailable(self.error_message.clone()))
    }

    fn name(&self) -> &str {
        "failing"
    }
}

#[tokio::test]
async fn test_coordinator_propagates_sink_errors() {
    let sink = Arc::new(FailingSink::new("test sink failure"));
    let coordinator = CloudEventsCoordinator::new(sink);

    let cloud_event = CloudEvent {
        r#type: "test.event".to_string(),
        data: None,
        extensions: Default::default(),
        id: None,
        source: None,
        subject: None,
    };

    let projection = create_cloudevents_projection("test", Uuid::new_v4(), vec![cloud_event]);

    let result = coordinator.process(&projection, None).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        SinkError::Unavailable(msg) => assert_eq!(msg, "test sink failure"),
        other => panic!("Expected Unavailable error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_envelope_serializes_to_valid_json() {
    let sink = Arc::new(RecordingSink::new());
    let coordinator = CloudEventsCoordinator::new(sink.clone());

    let cloud_event = CloudEvent {
        r#type: "order.created".to_string(),
        data: None,
        extensions: [("customext".to_string(), "value".to_string())]
            .into_iter()
            .collect(),
        id: Some("json-test-id".to_string()),
        source: Some("json-test-source".to_string()),
        subject: Some("json-test-subject".to_string()),
    };

    let projection = create_cloudevents_projection("orders", Uuid::new_v4(), vec![cloud_event]);
    coordinator.process(&projection, None).await.unwrap();

    let events = sink.get_events().await;
    let envelope = &events[0];

    // Serialize to JSON and verify structure
    let json = serde_json::to_string_pretty(envelope).expect("Should serialize to JSON");

    // Parse back to verify
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should parse JSON");

    assert_eq!(parsed["specversion"], "1.0");
    assert_eq!(parsed["id"], "json-test-id");
    assert_eq!(parsed["type"], "order.created");
    assert_eq!(parsed["source"], "json-test-source");
    assert_eq!(parsed["subject"], "json-test-subject");
    assert_eq!(parsed["customext"], "value");
}
