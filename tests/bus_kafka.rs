//! Kafka event bus integration tests using testcontainers.
//!
//! Run with: cargo test --test bus_kafka --features kafka -- --nocapture
//!
//! Uses Bitnami Kafka with KRaft mode (no Zookeeper) for simpler setup.

#![cfg(feature = "kafka")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use angzarr::bus::kafka::{KafkaEventBus, KafkaEventBusConfig};
use angzarr::bus::{BusError, EventBus, EventHandler};
use angzarr::dlq::{
    AngzarrDeadLetter, DeadLetterPayload, DlqConfig, EventProcessingFailedDetails,
    RejectionDetails, SequenceMismatchDetails,
};
use angzarr::proto::{event_page, CommandBook, Cover, EventBook, EventPage, MergeStrategy, Uuid};
use prost_types::Any;
use testcontainers::{
    core::{ContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};
use tokio::sync::mpsc;

/// Generates a unique port in the ephemeral range for testing.
/// Uses a simple hash of the current thread ID and time to get variety.
fn generate_test_port() -> u16 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    std::thread::current().id().hash(&mut hasher);
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .hash(&mut hasher);

    // Use ports in 29000-29999 range (less likely to conflict)
    29000 + (hasher.finish() % 1000) as u16
}

/// Start Kafka container using Redpanda with proper listener configuration.
///
/// The challenge with Kafka in testcontainers is that clients get broker addresses
/// from metadata, not from the bootstrap server connection. We solve this by:
///
/// 1. Using Redpanda which starts faster than traditional Kafka
/// 2. Generating a unique port and using fixed port mapping
/// 3. Configuring the advertised listener to match the fixed port
async fn start_kafka() -> (testcontainers::ContainerAsync<GenericImage>, String) {
    // Generate a unique port for this test
    let host_port = generate_test_port();
    let container_port = 9092u16;

    println!(
        "Starting Redpanda with fixed port mapping: {} -> {}",
        host_port, container_port
    );

    // Build advertised address with the fixed host port
    let advertised_addr = format!("localhost:{}", host_port);

    // Use Redpanda - Kafka-compatible, starts in ~5 seconds
    // Note: with_wait_for must be called before with_mapped_port due to type constraints
    let image = GenericImage::new("redpandadata/redpanda", "v24.1.1")
        .with_wait_for(WaitFor::message_on_stderr("Successfully started Redpanda"));

    let container = image
        .with_mapped_port(host_port, ContainerPort::Tcp(container_port))
        .with_cmd([
            "redpanda",
            "start",
            "--mode",
            "dev-container",
            "--smp",
            "1",
            "--memory",
            "512M",
            "--overprovisioned",
            "--kafka-addr",
            "0.0.0.0:9092",
            "--advertise-kafka-addr",
            &advertised_addr,
        ])
        .with_startup_timeout(Duration::from_secs(120))
        .start()
        .await
        .expect("Failed to start Redpanda container");

    // Wait for Redpanda to be fully ready
    tokio::time::sleep(Duration::from_secs(3)).await;

    let bootstrap_servers = format!("localhost:{}", host_port);
    println!("Kafka available at: {}", bootstrap_servers);

    (container, bootstrap_servers)
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
async fn test_kafka_publish_and_consume() {
    println!("=== Kafka Publish and Consume Test ===");
    println!("Starting Redpanda container...");

    let (_container, bootstrap_servers) = start_kafka().await;
    let domain = "test";
    let group_id = format!("test-group-{}", uuid::Uuid::new_v4());

    // Create publisher
    let publisher = KafkaEventBus::new(KafkaEventBusConfig::publisher(&bootstrap_servers))
        .await
        .expect("Failed to create publisher");

    // Create subscriber
    let subscriber = KafkaEventBus::new(KafkaEventBusConfig::subscriber(
        &bootstrap_servers,
        &group_id,
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

    // Give consumer time to start and join the group
    tokio::time::sleep(Duration::from_secs(2)).await;

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

    println!("=== Kafka Publish and Consume Test PASSED ===");
}

#[tokio::test]
async fn test_kafka_publisher_only() {
    println!("=== Kafka Publisher Only Test ===");
    println!("Starting Redpanda container...");

    let (_container, bootstrap_servers) = start_kafka().await;

    let publisher = KafkaEventBus::new(KafkaEventBusConfig::publisher(&bootstrap_servers))
        .await
        .expect("Failed to create publisher");

    let book = make_test_book("publish-test");

    // Should succeed
    publisher
        .publish(Arc::new(book))
        .await
        .expect("Publish should succeed");

    println!("=== Kafka Publisher Only Test PASSED ===");
}

#[tokio::test]
async fn test_kafka_multiple_messages() {
    println!("=== Kafka Multiple Messages Test ===");
    println!("Starting Redpanda container...");

    let (_container, bootstrap_servers) = start_kafka().await;
    let domain = "multi";
    let group_id = format!("test-multi-{}", uuid::Uuid::new_v4());

    let publisher = KafkaEventBus::new(KafkaEventBusConfig::publisher(&bootstrap_servers))
        .await
        .expect("Failed to create publisher");

    let subscriber = KafkaEventBus::new(KafkaEventBusConfig::subscriber(
        &bootstrap_servers,
        &group_id,
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
    tokio::time::sleep(Duration::from_secs(2)).await;

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

    println!("=== Kafka Multiple Messages Test PASSED ===");
}

#[tokio::test]
async fn test_kafka_dlq_publish() {
    use angzarr::dlq::create_publisher_async;

    println!("=== Kafka DLQ Publish Test ===");
    println!("Starting Redpanda container...");

    let (_container, bootstrap_servers) = start_kafka().await;

    // Create DLQ publisher
    let dlq_config = DlqConfig::kafka(&bootstrap_servers);
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

    println!("=== Kafka DLQ Publish Test PASSED ===");
}

#[tokio::test]
async fn test_kafka_dlq_sequence_mismatch() {
    use angzarr::dlq::create_publisher_async;

    println!("=== Kafka DLQ Sequence Mismatch Test ===");
    println!("Starting Redpanda container...");

    let (_container, bootstrap_servers) = start_kafka().await;

    // Create DLQ publisher
    let dlq_config = DlqConfig::kafka(&bootstrap_servers);
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

    println!("=== Kafka DLQ Sequence Mismatch Test PASSED ===");
}
