//! Tests for outbound CloudEvents and gRPC streaming.
//!
//! The outbound service bridges angzarr events to external systems:
//! 1. **CloudEvents sinks** - HTTP/Kafka/etc. delivery in CloudEvents format
//! 2. **gRPC streaming** - Real-time event streams filtered by correlation ID
//!
//! Key scenarios tested:
//! - Subscription lifecycle (create, deliver, cleanup on disconnect)
//! - Multi-subscriber fanout (same correlation ID)
//! - CloudEvent attribute mapping (type, source, extensions)
//! - Multi-page EventBook expansion (one CloudEvent per page)
//! - Content type parsing (JSON vs Protobuf)

use super::*;
use crate::handlers::projectors::cloudevents::sink::NullSink;
use crate::proto::{Cover, EventPage, Uuid as ProtoUuid};
use cloudevents::event::AttributesReader;
use tokio_stream::StreamExt;

// ============================================================================
// Test Helpers
// ============================================================================

fn make_test_event_book(correlation_id: &str) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: Some(ProtoUuid {
                value: uuid::Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: correlation_id.to_string(),
            edition: None,
            external_id: String::new(),
        }),
        pages: vec![EventPage {
            sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(0)),
            payload: Some(crate::proto::event_page::Payload::Event(prost_types::Any {
                type_url: "type.googleapis.com/test.Event".to_string(),
                value: vec![],
            })),
            created_at: None,
        }],
        snapshot: None,
        ..Default::default()
    }
}

fn make_multi_page_event_book(correlation_id: &str, page_count: usize) -> EventBook {
    let pages = (0..page_count)
        .map(|i| EventPage {
            sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(i as u32)),
            payload: Some(crate::proto::event_page::Payload::Event(prost_types::Any {
                type_url: format!("type.googleapis.com/test.Event{}", i),
                value: vec![],
            })),
            created_at: None,
        })
        .collect();

    EventBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: Some(ProtoUuid {
                value: uuid::Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: correlation_id.to_string(),
            edition: None,
            external_id: String::new(),
        }),
        pages,
        snapshot: None,
        ..Default::default()
    }
}

// ============================================================================
// gRPC Subscription Tests
// ============================================================================

/// Subscribe creates a subscription keyed by correlation ID.
///
/// Clients subscribe to receive events matching a correlation ID.
/// The subscription is stored until the client disconnects.
#[tokio::test]
async fn test_subscribe_creates_subscription() {
    let service = OutboundService::new();

    let filter = EventStreamFilter {
        correlation_id: "test-corr-id".to_string(),
    };

    let response = service.subscribe(Request::new(filter)).await.unwrap();
    let _stream = response.into_inner();

    // Verify subscription exists
    let subs = service.subscriptions.read().await;
    assert!(subs.contains_key("test-corr-id"));
    assert_eq!(subs.get("test-corr-id").unwrap().len(), 1);
}

/// Subscribe requires correlation ID — empty string rejected.
///
/// Correlation ID is the routing key. Without it, we can't filter events.
#[tokio::test]
async fn test_subscribe_requires_correlation_id() {
    let service = OutboundService::new();

    let filter = EventStreamFilter {
        correlation_id: String::new(),
    };

    let result = service.subscribe(Request::new(filter)).await;
    match result {
        Err(status) => assert_eq!(status.code(), tonic::Code::InvalidArgument),
        Ok(_) => panic!("Expected error for empty correlation_id"),
    }
}

/// Subscription cleanup on client disconnect.
///
/// When a gRPC stream is dropped (client disconnects), the subscription
/// is automatically removed. This prevents memory leaks from abandoned
/// subscriptions.
#[tokio::test]
async fn test_grpc_subscriber_cleanup_on_disconnect() {
    let service = OutboundService::new();

    let filter = EventStreamFilter {
        correlation_id: "cleanup-test".to_string(),
    };

    let response = service.subscribe(Request::new(filter)).await.unwrap();
    let stream = response.into_inner();

    // Verify subscription exists
    {
        let subs = service.subscriptions.read().await;
        assert!(subs.contains_key("cleanup-test"));
    }

    // Drop stream - simulates client disconnect
    drop(stream);

    // Give cleanup task time to run
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Verify subscription was cleaned up
    let subs = service.subscriptions.read().await;
    assert!(
        !subs.contains_key("cleanup-test"),
        "Subscription should be cleaned up after disconnect"
    );
}

// ============================================================================
// Event Delivery Tests
// ============================================================================

/// Events delivered to matching gRPC subscribers.
///
/// When an event is published with a correlation ID, all subscribers
/// to that ID receive the event.
#[tokio::test]
async fn test_event_delivery_to_grpc_subscriber() {
    let service = Arc::new(OutboundService::new());
    let handler = OutboundEventHandler::new(Arc::clone(&service));

    let filter = EventStreamFilter {
        correlation_id: "delivery-test".to_string(),
    };

    let response = service.subscribe(Request::new(filter)).await.unwrap();
    let mut stream = response.into_inner();

    // Deliver an event
    let book = Arc::new(make_test_event_book("delivery-test"));
    handler.handle(book).await.unwrap();

    // Verify event is received
    let received = tokio::time::timeout(tokio::time::Duration::from_millis(100), stream.next())
        .await
        .expect("Should receive event");

    assert!(received.is_some());
    let event_book = received.unwrap().unwrap();
    assert_eq!(
        event_book.cover.as_ref().unwrap().correlation_id,
        "delivery-test"
    );
}

/// Events without subscribers are silently dropped.
///
/// Not every event needs a real-time consumer. Events without matching
/// subscriptions are handled successfully but not delivered anywhere.
#[tokio::test]
async fn test_event_dropped_without_subscribers() {
    let service = Arc::new(OutboundService::new());
    let handler = OutboundEventHandler::new(Arc::clone(&service));

    // No subscribers registered for this correlation ID
    let book = Arc::new(make_test_event_book("no-subscriber"));

    // Should not error - events without subscribers are silently dropped
    let result = handler.handle(book).await;
    assert!(result.is_ok());

    // Verify no subscriptions were created
    let subs = service.subscriptions.read().await;
    assert!(!subs.contains_key("no-subscriber"));
}

/// Multiple subscribers to same correlation ID all receive events.
///
/// Fanout pattern: multiple clients can subscribe to the same correlation
/// ID and each receives a copy of every matching event.
#[tokio::test]
async fn test_multiple_subscribers_same_correlation() {
    let service = Arc::new(OutboundService::new());
    let handler = OutboundEventHandler::new(Arc::clone(&service));

    let filter1 = EventStreamFilter {
        correlation_id: "multi-sub".to_string(),
    };
    let filter2 = EventStreamFilter {
        correlation_id: "multi-sub".to_string(),
    };

    let response1 = service.subscribe(Request::new(filter1)).await.unwrap();
    let response2 = service.subscribe(Request::new(filter2)).await.unwrap();
    let mut stream1 = response1.into_inner();
    let mut stream2 = response2.into_inner();

    // Verify both subscriptions exist
    {
        let subs = service.subscriptions.read().await;
        assert_eq!(subs.get("multi-sub").unwrap().len(), 2);
    }

    // Deliver an event
    let book = Arc::new(make_test_event_book("multi-sub"));
    handler.handle(book).await.unwrap();

    // Both subscribers should receive the event
    let received1 = tokio::time::timeout(tokio::time::Duration::from_millis(100), stream1.next())
        .await
        .expect("Subscriber 1 should receive event");

    let received2 = tokio::time::timeout(tokio::time::Duration::from_millis(100), stream2.next())
        .await
        .expect("Subscriber 2 should receive event");

    assert!(received1.is_some());
    assert!(received2.is_some());
}

// ============================================================================
// CloudEvents Conversion Tests
// ============================================================================

/// EventBook wrapped as CloudEvent with proper attributes.
///
/// CloudEvents spec requires type, source, id. We map:
/// - type: "angzarr.{event_type}" prefix for namespace
/// - source: "angzarr/{domain}"
/// - correlationid: extension for cross-domain tracing
#[tokio::test]
async fn test_wrap_eventbook_as_cloudevent() {
    let book = make_test_event_book("test-corr");

    let event = wrap_eventbook_as_cloudevent(&book, 0).unwrap();

    // Verify CloudEvent attributes
    assert!(event.id().starts_with("test:"));
    assert_eq!(event.ty(), "angzarr.test.Event"); // Prefixed with "angzarr."
    assert_eq!(event.source().to_string(), "angzarr/test");
    assert!(event.data().is_some());

    // Verify correlation_id extension
    assert_eq!(
        event.extension("correlationid"),
        Some(&cloudevents::event::ExtensionValue::String(
            "test-corr".to_string()
        ))
    );
}

/// Multi-page EventBook produces one CloudEvent per page.
///
/// An EventBook can contain multiple events (batch from aggregate).
/// Each page becomes a separate CloudEvent for independent delivery.
#[tokio::test]
async fn test_multi_page_eventbook_produces_multiple_cloudevents() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingSink {
        count: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl CloudEventsSink for CountingSink {
        async fn publish(
            &self,
            events: Vec<CloudEventEnvelope>,
            _format: ContentType,
        ) -> Result<(), SinkError> {
            self.count.fetch_add(events.len(), Ordering::SeqCst);
            Ok(())
        }

        fn name(&self) -> &str {
            "counting"
        }
    }

    let sink = Arc::new(CountingSink {
        count: AtomicUsize::new(0),
    });
    let service = OutboundService::with_sinks(vec![sink.clone() as Arc<dyn CloudEventsSink>]);

    // Create multi-page EventBook
    let book = make_multi_page_event_book("test", 5);

    // Handle the book
    service.handle(&book).await.unwrap();

    // Should produce 5 CloudEvents (one per page)
    assert_eq!(sink.count.load(Ordering::SeqCst), 5);
}

// ============================================================================
// Sink Tests
// ============================================================================

/// Null sink accepts events without error (test double).
///
/// Used in tests when we don't need actual delivery.
#[tokio::test]
async fn test_outbound_service_with_null_sink() {
    let sink = Arc::new(NullSink);
    let service = OutboundService::with_sinks(vec![sink as Arc<dyn CloudEventsSink>]);

    let book = make_test_event_book("test");

    // Should succeed with null sink
    let result = service.handle(&book).await;
    assert!(result.is_ok());
}

// ============================================================================
// Utility Tests
// ============================================================================

/// Base64 encoding for protobuf content.
///
/// Standard RFC 4648 test vectors for base64 encoding used in
/// CloudEvent protobuf content.
#[test]
fn test_base64_encode() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

/// Content type parsing supports multiple aliases.
///
/// Operators can configure content type with various names.
/// Defaults to JSON for unknown types.
#[test]
fn test_content_type_parsing() {
    assert_eq!(ContentType::parse("json"), ContentType::Json);
    assert_eq!(ContentType::parse("JSON"), ContentType::Json);
    assert_eq!(ContentType::parse("protobuf"), ContentType::Protobuf);
    assert_eq!(ContentType::parse("proto"), ContentType::Protobuf);
    assert_eq!(ContentType::parse("pb"), ContentType::Protobuf);
    assert_eq!(ContentType::parse("unknown"), ContentType::Json);
}
