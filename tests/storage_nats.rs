//! NATS JetStream storage integration tests using testcontainers.
//!
//! Run with: cargo test --test storage_nats --features nats -- --nocapture
//!
//! These tests spin up NATS with JetStream in a container using testcontainers-rs.
//! No manual NATS setup required.

mod storage;

use std::time::Duration;

use angzarr::storage::nats::{NatsEventStore, NatsPositionStore, NatsSnapshotStore};
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};

/// Start NATS container with JetStream enabled.
///
/// Returns (container, client) where client is connected to the NATS server.
async fn start_nats() -> (
    testcontainers::ContainerAsync<GenericImage>,
    async_nats::Client,
) {
    let image = GenericImage::new("nats", "2.10")
        .with_exposed_port(4222.tcp())
        .with_wait_for(WaitFor::message_on_stderr(
            "Listening for client connections",
        ))
        .with_cmd(vec!["-js"]); // Enable JetStream

    let container = image
        .with_startup_timeout(Duration::from_secs(60))
        .start()
        .await
        .expect("Failed to start NATS container");

    let host_port = container
        .get_host_port_ipv4(4222)
        .await
        .expect("Failed to get mapped port");

    let host = container
        .get_host()
        .await
        .expect("Failed to get container host");

    let url = format!("nats://{}:{}", host, host_port);
    println!("NATS available at: {}", url);

    let client = async_nats::connect(&url)
        .await
        .expect("Failed to connect to NATS");

    (container, client)
}

fn test_prefix() -> String {
    format!(
        "test_{}",
        uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
    )
}

#[tokio::test]
async fn test_nats_event_store() {
    println!("=== NATS EventStore Tests ===");
    println!("Starting NATS container...");

    let (_container, client) = start_nats().await;
    let prefix = test_prefix();
    println!("Using test prefix: {}", prefix);

    let store = NatsEventStore::new(client, Some(&prefix))
        .await
        .expect("Failed to create NATS EventStore");

    run_event_store_tests!(&store);

    println!("=== All NATS EventStore tests PASSED ===");
}

#[tokio::test]
async fn test_nats_position_store() {
    println!("=== NATS PositionStore Tests ===");
    println!("Starting NATS container...");

    let (_container, client) = start_nats().await;
    let prefix = test_prefix();
    println!("Using test prefix: {}", prefix);

    let store = NatsPositionStore::new(client, Some(&prefix))
        .await
        .expect("Failed to create NATS PositionStore");

    run_position_store_tests!(&store);

    println!("=== All NATS PositionStore tests PASSED ===");
}

#[tokio::test]
async fn test_nats_snapshot_store() {
    println!("=== NATS SnapshotStore Tests ===");
    println!("Starting NATS container...");

    let (_container, client) = start_nats().await;
    let prefix = test_prefix();
    println!("Using test prefix: {}", prefix);

    let store = NatsSnapshotStore::new(client, Some(&prefix))
        .await
        .expect("Failed to create NATS SnapshotStore");

    run_snapshot_store_tests!(&store);

    println!("=== All NATS SnapshotStore tests PASSED ===");
}
