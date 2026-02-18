//! GCP Pub/Sub event bus integration tests using testcontainers.
//!
//! Run with: cargo test --test bus_pubsub --features pubsub -- --nocapture
//!
//! Uses the GCP Pub/Sub emulator for local testing.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use angzarr::bus::pubsub::{PubSubConfig, PubSubEventBus};
use angzarr::bus::{BusError, EventBus, EventHandler};
use angzarr::dlq::{
    AngzarrDeadLetter, DeadLetterPayload, DlqConfig, EventProcessingFailedDetails,
    RejectionDetails, SequenceMismatchDetails,
};
use angzarr::proto::{event_page, CommandBook, Cover, EventBook, EventPage, MergeStrategy, Uuid};
use prost_types::Any;
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};
use tokio::sync::mpsc;

/// Start GCP Pub/Sub emulator container.
///
/// Returns (container, emulator_host) where emulator_host is suitable for PUBSUB_EMULATOR_HOST.
async fn start_pubsub_emulator() -> (testcontainers::ContainerAsync<GenericImage>, String) {
    // Use the official gcloud CLI image with Pub/Sub emulator
    let image = GenericImage::new(
        "gcr.io/google.com/cloudsdktool/google-cloud-cli",
        "emulators",
    )
    .with_exposed_port(8085.tcp())
    .with_wait_for(WaitFor::message_on_stderr("Server started"));

    let container = image
        .with_cmd([
            "gcloud",
            "beta",
            "emulators",
            "pubsub",
            "start",
            "--host-port=0.0.0.0:8085",
        ])
        .with_startup_timeout(Duration::from_secs(120))
        .start()
        .await
        .expect("Failed to start pubsub emulator container");

    // Give emulator time to fully initialize
    tokio::time::sleep(Duration::from_secs(2)).await;

    let host_port = container
        .get_host_port_ipv4(8085)
        .await
        .expect("Failed to get mapped port");

    let host = container
        .get_host()
        .await
        .expect("Failed to get container host");

    let emulator_host = format!("{}:{}", host, host_port);

    println!("Pub/Sub emulator available at: {}", emulator_host);

    (container, emulator_host)
}

fn make_test_book(domain: &str) -> EventBook {
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

/// Handler that counts received events and sends to channel.
struct CountingHandler {
    count: Arc<AtomicUsize>,
    tx: mpsc::Sender<EventBook>,
}

impl EventHandler for CountingHandler {
    fn handle(
        &self,
        book: Arc<EventBook>,
    ) -> futures::future::BoxFuture<'static, std::result::Result<(), BusError>> {
        let count = self.count.clone();
        let tx = self.tx.clone();
        let book_clone = (*book).clone();
        Box::pin(async move {
            count.fetch_add(1, Ordering::SeqCst);
            let _ = tx.send(book_clone).await;
            Ok(())
        })
    }
}

#[tokio::test]
async fn test_pubsub_publish_and_consume() {
    println!("=== Pub/Sub Publish and Consume Test ===");
    println!("Starting Pub/Sub emulator container...");

    let (_container, emulator_host) = start_pubsub_emulator().await;
    let domain = "test";
    let subscription_id = format!("test-sub-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let project_id = "test-project";

    // Set emulator environment variable
    std::env::set_var("PUBSUB_EMULATOR_HOST", &emulator_host);

    // Create publisher
    let publisher = PubSubEventBus::new(PubSubConfig::publisher(project_id))
        .await
        .expect("Failed to create publisher");

    // Create subscriber
    let subscriber = PubSubEventBus::new(PubSubConfig::subscriber(
        project_id,
        &subscription_id,
        vec![domain.to_string()],
    ))
    .await
    .expect("Failed to create subscriber");

    // Set up counting handler
    let count = Arc::new(AtomicUsize::new(0));
    let (tx, mut rx) = mpsc::channel(10);
    subscriber
        .subscribe(Box::new(CountingHandler {
            count: count.clone(),
            tx,
        }))
        .await
        .expect("Failed to subscribe");

    subscriber
        .start_consuming()
        .await
        .expect("Failed to start consuming");

    // Give consumer time to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Publish event
    let book = make_test_book(domain);
    publisher
        .publish(Arc::new(book.clone()))
        .await
        .expect("Failed to publish");

    // Wait for message
    let received = tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("Timed out waiting for message")
        .expect("Channel closed");

    assert_eq!(received.cover.as_ref().unwrap().domain, domain);
    assert_eq!(count.load(Ordering::SeqCst), 1);

    // Clean up env var
    std::env::remove_var("PUBSUB_EMULATOR_HOST");

    println!("=== Pub/Sub Publish and Consume Test PASSED ===");
}

#[tokio::test]
async fn test_pubsub_publisher_only() {
    println!("=== Pub/Sub Publisher Only Test ===");
    println!("Starting Pub/Sub emulator container...");

    let (_container, emulator_host) = start_pubsub_emulator().await;
    let project_id = "test-project";

    // Set emulator environment variable
    std::env::set_var("PUBSUB_EMULATOR_HOST", &emulator_host);

    let publisher = PubSubEventBus::new(PubSubConfig::publisher(project_id))
        .await
        .expect("Failed to create publisher");

    let book = make_test_book("publish-test");

    // Should succeed (topic created automatically)
    publisher
        .publish(Arc::new(book))
        .await
        .expect("Publish should succeed");

    // Clean up env var
    std::env::remove_var("PUBSUB_EMULATOR_HOST");

    println!("=== Pub/Sub Publisher Only Test PASSED ===");
}

#[tokio::test]
async fn test_pubsub_multiple_messages() {
    println!("=== Pub/Sub Multiple Messages Test ===");
    println!("Starting Pub/Sub emulator container...");

    let (_container, emulator_host) = start_pubsub_emulator().await;
    let domain = "multi";
    let subscription_id = format!("test-multi-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let project_id = "test-project";

    // Set emulator environment variable
    std::env::set_var("PUBSUB_EMULATOR_HOST", &emulator_host);

    let publisher = PubSubEventBus::new(PubSubConfig::publisher(project_id))
        .await
        .expect("Failed to create publisher");

    let subscriber = PubSubEventBus::new(PubSubConfig::subscriber(
        project_id,
        &subscription_id,
        vec![domain.to_string()],
    ))
    .await
    .expect("Failed to create subscriber");

    let count = Arc::new(AtomicUsize::new(0));
    let (tx, mut rx) = mpsc::channel(100);
    subscriber
        .subscribe(Box::new(CountingHandler {
            count: count.clone(),
            tx,
        }))
        .await
        .unwrap();

    subscriber.start_consuming().await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Publish 5 messages
    for _ in 0..5 {
        let book = make_test_book(domain);
        publisher.publish(Arc::new(book)).await.unwrap();
    }

    // Wait for all messages
    for _ in 0..5 {
        tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("Timed out")
            .expect("Channel closed");
    }

    assert_eq!(count.load(Ordering::SeqCst), 5);

    // Clean up env var
    std::env::remove_var("PUBSUB_EMULATOR_HOST");

    println!("=== Pub/Sub Multiple Messages Test PASSED ===");
}

#[tokio::test]
async fn test_pubsub_dlq_publish() {
    use angzarr::dlq::create_publisher_async;

    println!("=== Pub/Sub DLQ Publish Test ===");
    println!("Starting Pub/Sub emulator container...");

    let (_container, emulator_host) = start_pubsub_emulator().await;

    // Set emulator environment variable
    std::env::set_var("PUBSUB_EMULATOR_HOST", &emulator_host);

    // Create DLQ publisher
    let dlq_config = DlqConfig::pubsub();
    let dlq_publisher = create_publisher_async(&dlq_config)
        .await
        .expect("Failed to create DLQ publisher");

    // Create a dead letter from an event processing failure
    let book = make_test_book("orders");
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

    // Publish should succeed
    dlq_publisher
        .publish(dead_letter)
        .await
        .expect("DLQ publish should succeed");

    // Clean up env var
    std::env::remove_var("PUBSUB_EMULATOR_HOST");

    println!("=== Pub/Sub DLQ Publish Test PASSED ===");
}

#[tokio::test]
async fn test_pubsub_dlq_sequence_mismatch() {
    use angzarr::dlq::create_publisher_async;

    println!("=== Pub/Sub DLQ Sequence Mismatch Test ===");
    println!("Starting Pub/Sub emulator container...");

    let (_container, emulator_host) = start_pubsub_emulator().await;

    // Set emulator environment variable
    std::env::set_var("PUBSUB_EMULATOR_HOST", &emulator_host);

    // Create DLQ publisher
    let dlq_config = DlqConfig::pubsub();
    let dlq_publisher = create_publisher_async(&dlq_config)
        .await
        .expect("Failed to create DLQ publisher");

    // Create a command book
    let command_book = CommandBook {
        cover: Some(Cover {
            domain: "inventory".to_string(),
            root: Some(Uuid {
                value: uuid::Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: format!("txn-{}", uuid::Uuid::new_v4()),
            edition: None,
        }),
        pages: vec![],
        ..Default::default()
    };

    // Create a dead letter for sequence mismatch
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

    // Clean up env var
    std::env::remove_var("PUBSUB_EMULATOR_HOST");

    println!("=== Pub/Sub DLQ Sequence Mismatch Test PASSED ===");
}
