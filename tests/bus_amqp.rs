//! AMQP/RabbitMQ event bus integration tests using testcontainers.
//!
//! Run with: cargo test --test bus_amqp --features amqp -- --nocapture
//!
//! These tests spin up RabbitMQ in a container using testcontainers-rs.
//! No manual RabbitMQ setup required.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use angzarr::bus::amqp::{AmqpConfig, AmqpEventBus};
use angzarr::bus::{BusError, EventBus, EventHandler};
use angzarr::dlq::{
    AngzarrDeadLetter, DeadLetterPayload, DlqConfig, EventProcessingFailedDetails, RejectionDetails,
};
use angzarr::proto::{event_page::Sequence, CommandBook, Cover, EventBook, EventPage, Uuid};
use prost_types::Any;
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};
use tokio::sync::mpsc;

/// Start RabbitMQ container.
///
/// Returns (container, amqp_url) where amqp_url is suitable for AMQP connection.
async fn start_rabbitmq() -> (testcontainers::ContainerAsync<GenericImage>, String) {
    let image = GenericImage::new("rabbitmq", "3-management")
        .with_exposed_port(5672.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Server startup complete"));

    let container = image
        .with_startup_timeout(Duration::from_secs(60))
        .start()
        .await
        .expect("Failed to start rabbitmq container");

    // Brief delay to ensure RabbitMQ is fully ready
    tokio::time::sleep(Duration::from_secs(2)).await;

    let host_port = container
        .get_host_port_ipv4(5672)
        .await
        .expect("Failed to get mapped port");

    let host = container
        .get_host()
        .await
        .expect("Failed to get container host");

    let amqp_url = format!("amqp://guest:guest@{}:{}", host, host_port);

    println!("RabbitMQ available at: {}", amqp_url);

    (container, amqp_url)
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
            sequence: Some(Sequence::Num(0)),
            created_at: None,
            external_payload: None,
            event: Some(Any {
                type_url: "type.googleapis.com/test.TestEvent".to_string(),
                value: vec![1, 2, 3],
            }),
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
async fn test_publish_and_consume() {
    println!("=== AMQP Publish and Consume Test ===");
    println!("Starting RabbitMQ container...");

    let (_container, url) = start_rabbitmq().await;
    let queue_name = format!("test-queue-{}", uuid::Uuid::new_v4());

    // Create publisher
    let publisher = AmqpEventBus::new(AmqpConfig::publisher(&url))
        .await
        .expect("Failed to create publisher");

    // Create subscriber
    let subscriber = AmqpEventBus::new(AmqpConfig::subscriber(&url, &queue_name, "test"))
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
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Publish event
    let book = make_test_book("test");
    publisher
        .publish(Arc::new(book.clone()))
        .await
        .expect("Failed to publish");

    // Wait for message
    let received = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("Timed out waiting for message")
        .expect("Channel closed");

    assert_eq!(received.cover.as_ref().unwrap().domain, "test");
    assert_eq!(count.load(Ordering::SeqCst), 1);

    println!("=== AMQP Publish and Consume Test PASSED ===");
}

#[tokio::test]
async fn test_publisher_retries_on_failure() {
    println!("=== AMQP Publisher Retries Test ===");
    println!("Starting RabbitMQ container...");

    let (_container, url) = start_rabbitmq().await;

    let publisher = AmqpEventBus::new(AmqpConfig::publisher(&url))
        .await
        .expect("Failed to create publisher");

    let book = make_test_book("retry-test");

    // Should succeed on first try
    publisher
        .publish(Arc::new(book))
        .await
        .expect("Publish should succeed");

    println!("=== AMQP Publisher Retries Test PASSED ===");
}

#[tokio::test]
async fn test_consumer_receives_multiple_messages() {
    println!("=== AMQP Multiple Messages Test ===");
    println!("Starting RabbitMQ container...");

    let (_container, url) = start_rabbitmq().await;
    let queue_name = format!("test-multi-{}", uuid::Uuid::new_v4());

    let publisher = AmqpEventBus::new(AmqpConfig::publisher(&url))
        .await
        .expect("Failed to create publisher");

    let subscriber = AmqpEventBus::new(AmqpConfig::subscriber(&url, &queue_name, "multi"))
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
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Publish 10 messages
    for _ in 0..10 {
        let book = make_test_book("multi");
        publisher.publish(Arc::new(book)).await.unwrap();
    }

    // Wait for all messages
    for _ in 0..10 {
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("Timed out")
            .expect("Channel closed");
    }

    assert_eq!(count.load(Ordering::SeqCst), 10);

    println!("=== AMQP Multiple Messages Test PASSED ===");
}

#[tokio::test]
async fn test_dlq_publish() {
    use angzarr::dlq::create_publisher_async;

    println!("=== AMQP DLQ Publish Test ===");
    println!("Starting RabbitMQ container...");

    let (_container, url) = start_rabbitmq().await;

    // Create DLQ publisher
    let dlq_config = DlqConfig::amqp(&url);
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

    println!("=== AMQP DLQ Publish Test PASSED ===");
}

#[tokio::test]
async fn test_dlq_command_sequence_mismatch() {
    use angzarr::dlq::{create_publisher_async, SequenceMismatchDetails};
    use angzarr::proto::MergeStrategy;

    println!("=== AMQP DLQ Sequence Mismatch Test ===");
    println!("Starting RabbitMQ container...");

    let (_container, url) = start_rabbitmq().await;

    // Create DLQ publisher
    let dlq_config = DlqConfig::amqp(&url);
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

    println!("=== AMQP DLQ Sequence Mismatch Test PASSED ===");
}
