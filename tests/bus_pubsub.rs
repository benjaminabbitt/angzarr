//! GCP Pub/Sub event bus integration tests using testcontainers.
//!
//! Run with: cargo test --test bus_pubsub --features "pubsub test-utils" -- --nocapture
//!
//! Uses the GCP Pub/Sub emulator for local testing.

#![cfg(feature = "pubsub")]

mod bus;

use std::time::Duration;

use angzarr::bus::pubsub::{PubSubConfig, PubSubEventBus};
use angzarr::dlq::DlqConfig;
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};

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

fn test_prefix() -> String {
    format!(
        "test_{}",
        uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
    )
}

#[tokio::test]
async fn test_pubsub_event_bus() {
    println!("=== Pub/Sub EventBus Tests ===");

    let (_container, emulator_host) = start_pubsub_emulator().await;
    let prefix = test_prefix();

    // Set emulator environment
    std::env::set_var("PUBSUB_EMULATOR_HOST", &emulator_host);

    let bus = PubSubEventBus::new(PubSubConfig::publisher("test-project"))
        .await
        .expect("Failed to create Pub/Sub publisher");

    run_event_bus_tests!(&bus, &prefix);

    println!("=== All Pub/Sub EventBus tests PASSED ===");
}

#[tokio::test]
async fn test_pubsub_dlq() {
    println!("=== Pub/Sub DLQ Tests ===");

    let (_container, emulator_host) = start_pubsub_emulator().await;

    // Set emulator environment
    std::env::set_var("PUBSUB_EMULATOR_HOST", &emulator_host);

    let dlq_config = DlqConfig::pubsub();

    bus::event_bus_tests::test_dlq_publish(&dlq_config).await;
    println!("  test_dlq_publish: PASSED");

    bus::event_bus_tests::test_dlq_sequence_mismatch(&dlq_config).await;
    println!("  test_dlq_sequence_mismatch: PASSED");

    println!("=== All Pub/Sub DLQ tests PASSED ===");
}
