//! AWS SNS/SQS event bus integration tests using testcontainers.
//!
//! Run with: cargo test --test bus_sns_sqs --features sns-sqs -- --nocapture
//!
//! Uses LocalStack to emulate AWS SNS/SQS locally.
//! Tests share a single LocalStack container to avoid rootless port conflicts.

#![cfg(feature = "sns-sqs")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use angzarr::bus::sns_sqs::{SnsSqsConfig, SnsSqsEventBus};
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
    ContainerAsync, GenericImage, ImageExt,
};
use tokio::sync::{mpsc, OnceCell};

/// Shared LocalStack container and endpoint URL.
/// Using a shared container avoids rootless port conflicts in podman
/// that occur when rapidly starting/stopping containers.
static LOCALSTACK: OnceCell<(ContainerAsync<GenericImage>, String)> = OnceCell::const_new();

/// Get the shared LocalStack endpoint, starting the container if needed.
async fn get_localstack_endpoint() -> String {
    let (_, endpoint) = LOCALSTACK
        .get_or_init(|| async {
            println!("Starting shared LocalStack container...");
            let (container, endpoint) = start_localstack_internal().await;
            println!("LocalStack available at: {}", endpoint);
            (container, endpoint)
        })
        .await;
    endpoint.clone()
}

/// Start LocalStack container with SNS/SQS services (internal implementation).
///
/// Returns (container, endpoint_url) where endpoint_url is suitable for AWS SDK connection.
async fn start_localstack_internal() -> (ContainerAsync<GenericImage>, String) {
    let image = GenericImage::new("localstack/localstack", "latest")
        .with_exposed_port(4566.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Ready."));

    let container = image
        .with_env_var("SERVICES", "sns,sqs")
        .with_env_var("AWS_DEFAULT_REGION", "us-east-1")
        .with_env_var("EAGER_SERVICE_LOADING", "1")
        .with_env_var("DISABLE_EVENTS", "1") // Disable analytics
        .with_env_var("SKIP_INFRA_DOWNLOADS", "1") // Don't download additional components
        .with_env_var("DNS_DISABLED", "true") // Disable DNS to avoid conflicts
        .with_env_var("LOCALSTACK_HOST", "localhost") // Use localhost for all service URLs
        .with_startup_timeout(Duration::from_secs(180))
        .start()
        .await
        .expect("Failed to start localstack container");

    // Give LocalStack time to fully initialize services
    // SNS/SQS services need extra time to be fully ready
    tokio::time::sleep(Duration::from_secs(5)).await;

    let host_port = container
        .get_host_port_ipv4(4566)
        .await
        .expect("Failed to get mapped port");

    let host = container
        .get_host()
        .await
        .expect("Failed to get container host");

    let endpoint_url = format!("http://{}:{}", host, host_port);

    println!("LocalStack (SNS/SQS) available at: {}", endpoint_url);

    (container, endpoint_url)
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
async fn test_sns_sqs_publish_and_consume() {
    println!("=== SNS/SQS Publish and Consume Test ===");

    let endpoint_url = get_localstack_endpoint().await;
    let domain = "test-pub-consume";
    let subscription_id = format!("test-sub-{}", uuid::Uuid::new_v4());

    // Set dummy AWS credentials for LocalStack
    std::env::set_var("AWS_ACCESS_KEY_ID", "test");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
    std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");

    // Create publisher
    let publisher = SnsSqsEventBus::new(
        SnsSqsConfig::publisher()
            .with_endpoint(&endpoint_url)
            .with_region("us-east-1"),
    )
    .await
    .expect("Failed to create publisher");

    // Create subscriber
    let subscriber = SnsSqsEventBus::new(
        SnsSqsConfig::subscriber(&subscription_id, vec![domain.to_string()])
            .with_endpoint(&endpoint_url)
            .with_region("us-east-1"),
    )
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

    println!("=== SNS/SQS Publish and Consume Test PASSED ===");
}

#[tokio::test]
async fn test_sns_sqs_publisher_only() {
    println!("=== SNS/SQS Publisher Only Test ===");

    let endpoint_url = get_localstack_endpoint().await;

    // Set dummy AWS credentials for LocalStack
    std::env::set_var("AWS_ACCESS_KEY_ID", "test");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
    std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");

    let publisher = SnsSqsEventBus::new(
        SnsSqsConfig::publisher()
            .with_endpoint(&endpoint_url)
            .with_region("us-east-1"),
    )
    .await
    .expect("Failed to create publisher");

    // Use unique domain name per test
    let domain = format!("pub-only-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let book = make_test_book(&domain);

    // Should succeed (topic created automatically)
    publisher
        .publish(Arc::new(book))
        .await
        .expect("Publish should succeed");

    println!("=== SNS/SQS Publisher Only Test PASSED ===");
}

#[tokio::test]
async fn test_sns_sqs_multiple_messages() {
    println!("=== SNS/SQS Multiple Messages Test ===");

    let endpoint_url = get_localstack_endpoint().await;
    // Use unique domain name per test to avoid collisions with shared container
    let domain = format!("multi-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let subscription_id = format!("test-multi-{}", &uuid::Uuid::new_v4().to_string()[..8]);

    // Set dummy AWS credentials for LocalStack
    std::env::set_var("AWS_ACCESS_KEY_ID", "test");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
    std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");

    let publisher = SnsSqsEventBus::new(
        SnsSqsConfig::publisher()
            .with_endpoint(&endpoint_url)
            .with_region("us-east-1"),
    )
    .await
    .expect("Failed to create publisher");

    let subscriber = SnsSqsEventBus::new(
        SnsSqsConfig::subscriber(&subscription_id, vec![domain.clone()])
            .with_endpoint(&endpoint_url)
            .with_region("us-east-1"),
    )
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
        let book = make_test_book(&domain);
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

    println!("=== SNS/SQS Multiple Messages Test PASSED ===");
}

#[tokio::test]
async fn test_sns_sqs_dlq_publish() {
    use angzarr::dlq::create_publisher_async;

    println!("=== SNS/SQS DLQ Publish Test ===");

    let endpoint_url = get_localstack_endpoint().await;

    // Set dummy AWS credentials for LocalStack
    std::env::set_var("AWS_ACCESS_KEY_ID", "test");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
    std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");

    // Create DLQ publisher
    let dlq_config = DlqConfig::sns_sqs()
        .with_aws_region("us-east-1")
        .with_aws_endpoint(&endpoint_url);
    let dlq_publisher = create_publisher_async(&dlq_config)
        .await
        .expect("Failed to create DLQ publisher");

    // Create a dead letter from an event processing failure
    let domain = format!("dlq-orders-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let book = make_test_book(&domain);
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

    println!("=== SNS/SQS DLQ Publish Test PASSED ===");
}

#[tokio::test]
async fn test_sns_sqs_dlq_sequence_mismatch() {
    use angzarr::dlq::create_publisher_async;

    println!("=== SNS/SQS DLQ Sequence Mismatch Test ===");

    let endpoint_url = get_localstack_endpoint().await;

    // Set dummy AWS credentials for LocalStack
    std::env::set_var("AWS_ACCESS_KEY_ID", "test");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
    std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");

    // Create DLQ publisher
    let dlq_config = DlqConfig::sns_sqs()
        .with_aws_region("us-east-1")
        .with_aws_endpoint(&endpoint_url);
    let dlq_publisher = create_publisher_async(&dlq_config)
        .await
        .expect("Failed to create DLQ publisher");

    // Create a command book with unique domain
    let domain = format!("dlq-inv-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let command_book = CommandBook {
        cover: Some(Cover {
            domain,
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

    println!("=== SNS/SQS DLQ Sequence Mismatch Test PASSED ===");
}
