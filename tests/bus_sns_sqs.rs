//! AWS SNS/SQS event bus contract tests using testcontainers.
//!
//! Run with: cargo test --test bus_sns_sqs --features "sns-sqs test-utils" -- --nocapture
//!
//! These tests verify that the SNS/SQS bus implementation correctly fulfills
//! the EventBus trait contract. Uses LocalStack to emulate AWS SNS/SQS locally.
//! Tests share a single LocalStack container to avoid rootless port conflicts.

#![cfg(feature = "sns-sqs")]

mod bus;

use std::time::Duration;

use angzarr::bus::sns_sqs::{SnsSqsConfig, SnsSqsEventBus};
use angzarr::dlq::DlqConfig;
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    ContainerAsync, GenericImage, ImageExt,
};
use tokio::sync::OnceCell;

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

fn test_prefix() -> String {
    format!(
        "test_{}",
        uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
    )
}

#[tokio::test]
async fn test_sns_sqs_event_bus() {
    println!("=== SNS/SQS EventBus Tests ===");

    let endpoint_url = get_localstack_endpoint().await;
    let prefix = test_prefix();

    // Set dummy AWS credentials for LocalStack
    std::env::set_var("AWS_ACCESS_KEY_ID", "test");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
    std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");

    let bus = SnsSqsEventBus::new(
        SnsSqsConfig::publisher()
            .with_endpoint(&endpoint_url)
            .with_region("us-east-1"),
    )
    .await
    .expect("Failed to create SNS/SQS publisher");

    run_event_bus_tests!(&bus, &prefix);

    println!("=== All SNS/SQS EventBus tests PASSED ===");
}

#[tokio::test]
async fn test_sns_sqs_dlq() {
    println!("=== SNS/SQS DLQ Tests ===");

    let endpoint_url = get_localstack_endpoint().await;

    // Set dummy AWS credentials for LocalStack
    std::env::set_var("AWS_ACCESS_KEY_ID", "test");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
    std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");

    let dlq_config = DlqConfig::sns_sqs()
        .with_aws_region("us-east-1")
        .with_aws_endpoint(&endpoint_url);

    bus::event_bus_tests::test_dlq_publish(&dlq_config).await;
    println!("  test_dlq_publish: PASSED");

    bus::event_bus_tests::test_dlq_sequence_mismatch(&dlq_config).await;
    println!("  test_dlq_sequence_mismatch: PASSED");

    println!("=== All SNS/SQS DLQ tests PASSED ===");
}
