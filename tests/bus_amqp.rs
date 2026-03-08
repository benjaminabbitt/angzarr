//! AMQP/RabbitMQ event bus contract tests using testcontainers.
//!
//! Run with: cargo test --test bus_amqp --features "amqp test-utils" -- --nocapture
//!
//! These tests verify that the AMQP bus implementation correctly fulfills
//! the EventBus trait contract. Uses testcontainers-rs to spin up RabbitMQ.
//! No manual RabbitMQ setup required.

#![cfg(feature = "amqp")]

mod bus;

use std::time::Duration;

use angzarr::bus::amqp::{AmqpConfig, AmqpEventBus};
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};

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

fn test_prefix() -> String {
    format!(
        "test_{}",
        uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
    )
}

#[tokio::test]
async fn test_amqp_event_bus() {
    println!("=== AMQP EventBus Tests ===");
    println!("Starting RabbitMQ container...");

    let (_container, url) = start_rabbitmq().await;
    let prefix = test_prefix();

    let bus = AmqpEventBus::new(AmqpConfig::publisher(&url))
        .await
        .expect("Failed to create AMQP publisher");

    // Run shared tests (without DLQ - those need separate container lifetime)
    run_event_bus_tests!(&bus, &prefix);

    println!("=== All AMQP EventBus tests PASSED ===");
}
