//! EventBus interface tests.
//!
//! These tests verify the contract of the EventBus trait.
//! Each bus implementation should run these tests.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use angzarr::bus::EventBus;
use angzarr::dlq::{
    create_publisher_async, AngzarrDeadLetter, DeadLetterPayload, DlqConfig,
    EventProcessingFailedDetails, RejectionDetails, SequenceMismatchDetails,
};
use angzarr::proto::{event_page, CommandBook, Cover, EventBook, EventPage, MergeStrategy, Uuid};
use angzarr::test_utils::CapturingHandler;
use prost_types::Any;
use tokio::sync::mpsc;

/// Create a test EventBook for a given domain.
pub fn make_event_book(domain: &str) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(Uuid {
                value: uuid::Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: format!("test-{}", uuid::Uuid::new_v4()),
            edition: None,
        }),
        pages: vec![EventPage {
            sequence: 0,
            created_at: None,
            payload: Some(event_page::Payload::Event(Any {
                type_url: "type.googleapis.com/test.TestEvent".to_string(),
                value: vec![1, 2, 3],
            })),
        }],
        snapshot: None,
        ..Default::default()
    }
}

/// Create a test CommandBook for DLQ tests.
pub fn make_command_book(domain: &str) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(Uuid {
                value: uuid::Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: format!("txn-{}", uuid::Uuid::new_v4()),
            edition: None,
        }),
        pages: vec![],
        ..Default::default()
    }
}

// =============================================================================
// EventBus publish/subscribe tests
// =============================================================================

/// Test basic publish and subscribe roundtrip.
pub async fn test_publish_subscribe_roundtrip<B: EventBus>(
    publisher: &B,
    domain: &str,
    subscriber_name: &str,
) {
    let subscriber = publisher
        .create_subscriber(subscriber_name, Some(domain))
        .await
        .expect("Failed to create subscriber");

    let count = Arc::new(AtomicUsize::new(0));
    let (tx, mut rx) = mpsc::channel(10);

    subscriber
        .subscribe(Box::new(CapturingHandler::with_count(tx, count.clone())))
        .await
        .expect("Failed to subscribe");

    subscriber
        .start_consuming()
        .await
        .expect("Failed to start consuming");

    // Give consumer time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Publish event
    let book = make_event_book(domain);
    publisher
        .publish(Arc::new(book.clone()))
        .await
        .expect("Failed to publish");

    // Wait for message
    let received = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("Timed out waiting for message")
        .expect("Channel closed");

    assert_eq!(
        received.cover.as_ref().unwrap().domain,
        domain,
        "Received event should have correct domain"
    );
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "Handler should be called once"
    );
}

/// Test that publish works without any subscribers.
pub async fn test_publish_only<B: EventBus>(publisher: &B, domain: &str) {
    let book = make_event_book(domain);

    // Should succeed without error
    publisher
        .publish(Arc::new(book))
        .await
        .expect("Publish should succeed without subscribers");
}

/// Test receiving multiple messages.
pub async fn test_multiple_messages<B: EventBus>(
    publisher: &B,
    domain: &str,
    subscriber_name: &str,
    message_count: usize,
) {
    let subscriber = publisher
        .create_subscriber(subscriber_name, Some(domain))
        .await
        .expect("Failed to create subscriber");

    let count = Arc::new(AtomicUsize::new(0));
    let (tx, mut rx) = mpsc::channel(100);

    subscriber
        .subscribe(Box::new(CapturingHandler::with_count(tx, count.clone())))
        .await
        .expect("Failed to subscribe");

    subscriber
        .start_consuming()
        .await
        .expect("Failed to start consuming");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Publish multiple messages
    for _ in 0..message_count {
        let book = make_event_book(domain);
        publisher
            .publish(Arc::new(book))
            .await
            .expect("Failed to publish");
    }

    // Wait for all messages
    for i in 0..message_count {
        tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect(&format!("Timed out waiting for message {}", i))
            .expect("Channel closed");
    }

    assert_eq!(
        count.load(Ordering::SeqCst),
        message_count,
        "Handler should be called {} times",
        message_count
    );
}

/// Test domain filtering - subscriber should only receive events for its domain.
pub async fn test_domain_filtering<B: EventBus>(
    publisher: &B,
    target_domain: &str,
    other_domain: &str,
    subscriber_name: &str,
) {
    let subscriber = publisher
        .create_subscriber(subscriber_name, Some(target_domain))
        .await
        .expect("Failed to create subscriber");

    let (tx, mut rx) = mpsc::channel(10);

    subscriber
        .subscribe(Box::new(CapturingHandler::new(tx)))
        .await
        .expect("Failed to subscribe");

    subscriber
        .start_consuming()
        .await
        .expect("Failed to start consuming");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Publish to target domain - should be received
    publisher
        .publish(Arc::new(make_event_book(target_domain)))
        .await
        .expect("Failed to publish to target domain");

    // Publish to other domain - should NOT be received
    publisher
        .publish(Arc::new(make_event_book(other_domain)))
        .await
        .expect("Failed to publish to other domain");

    // Should receive only the target domain event
    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("Timed out waiting for target domain event")
        .expect("Channel closed");

    assert_eq!(
        received.cover.as_ref().unwrap().domain,
        target_domain,
        "Should receive event from target domain"
    );

    // Should NOT receive another event (other domain was filtered)
    let timeout_result: Result<Option<EventBook>, _> =
        tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(
        timeout_result.is_err(),
        "Should not receive event from other domain"
    );
}

// =============================================================================
// DLQ tests
// =============================================================================

/// Test DLQ publish with event processing failure.
pub async fn test_dlq_publish(dlq_config: &DlqConfig) {
    let dlq_publisher = create_publisher_async(dlq_config)
        .await
        .expect("Failed to create DLQ publisher");

    let book = make_event_book("orders");
    let dead_letter = AngzarrDeadLetter {
        cover: book.cover.clone(),
        payload: DeadLetterPayload::Events(book),
        rejection_reason: "Handler threw an exception".to_string(),
        rejection_details: Some(RejectionDetails::EventProcessingFailed(
            EventProcessingFailedDetails {
                error: "Connection refused".to_string(),
                retry_count: 3,
                is_transient: false,
            },
        )),
        source_component: "saga-order-fulfillment".to_string(),
        source_component_type: "saga".to_string(),
        occurred_at: None,
        metadata: std::collections::HashMap::new(),
    };

    dlq_publisher
        .publish(dead_letter)
        .await
        .expect("DLQ publish should succeed");
}

/// Test DLQ publish with sequence mismatch rejection.
pub async fn test_dlq_sequence_mismatch(dlq_config: &DlqConfig) {
    let dlq_publisher = create_publisher_async(dlq_config)
        .await
        .expect("Failed to create DLQ publisher");

    let command_book = make_command_book("inventory");
    let dead_letter = AngzarrDeadLetter {
        cover: command_book.cover.clone(),
        payload: DeadLetterPayload::Command(command_book),
        rejection_reason: "Sequence mismatch".to_string(),
        rejection_details: Some(RejectionDetails::SequenceMismatch(
            SequenceMismatchDetails {
                expected_sequence: 0,
                actual_sequence: 5,
                merge_strategy: MergeStrategy::MergeManual,
            },
        )),
        source_component: "aggregate-inventory".to_string(),
        source_component_type: "aggregate".to_string(),
        occurred_at: None,
        metadata: std::collections::HashMap::new(),
    };

    dlq_publisher
        .publish(dead_letter)
        .await
        .expect("DLQ publish should succeed");
}

// =============================================================================
// Test runner macro
// =============================================================================

/// Run all EventBus interface tests against a bus implementation.
///
/// # Arguments
/// * `$publisher` - The bus instance configured as publisher
/// * `$prefix` - Unique prefix for subscriber names (for test isolation)
/// * `$dlq_config` - Optional DlqConfig for DLQ tests
#[macro_export]
macro_rules! run_event_bus_tests {
    ($publisher:expr, $prefix:expr) => {
        use $crate::bus::event_bus_tests::*;

        // Publish/subscribe roundtrip
        test_publish_subscribe_roundtrip(
            $publisher,
            &format!("{}-roundtrip", $prefix),
            &format!("{}-sub-roundtrip", $prefix),
        )
        .await;
        println!("  test_publish_subscribe_roundtrip: PASSED");

        // Publish only
        test_publish_only($publisher, &format!("{}-publish-only", $prefix)).await;
        println!("  test_publish_only: PASSED");

        // Multiple messages
        test_multiple_messages(
            $publisher,
            &format!("{}-multi", $prefix),
            &format!("{}-sub-multi", $prefix),
            5,
        )
        .await;
        println!("  test_multiple_messages: PASSED");

        // Domain filtering
        test_domain_filtering(
            $publisher,
            &format!("{}-target", $prefix),
            &format!("{}-other", $prefix),
            &format!("{}-sub-filter", $prefix),
        )
        .await;
        println!("  test_domain_filtering: PASSED");
    };

    ($publisher:expr, $prefix:expr, $dlq_config:expr) => {
        // Run base tests
        run_event_bus_tests!($publisher, $prefix);

        // DLQ tests
        test_dlq_publish($dlq_config).await;
        println!("  test_dlq_publish: PASSED");

        test_dlq_sequence_mismatch($dlq_config).await;
        println!("  test_dlq_sequence_mismatch: PASSED");
    };
}
