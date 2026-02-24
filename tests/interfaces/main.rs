//! Contract tests for storage and bus backends using Cucumber/Gherkin.
//!
//! These tests verify that all implementations correctly fulfill their trait contracts.
//! Select a backend via environment variable:
//!
//! ## Storage Tests
//! ```bash
//! # SQLite (default)
//! cargo test --test interfaces --features sqlite
//!
//! # PostgreSQL (uses testcontainers)
//! STORAGE_BACKEND=postgres cargo test --test interfaces --features postgres
//!
//! # Redis (uses testcontainers)
//! STORAGE_BACKEND=redis cargo test --test interfaces --features redis
//!
//! # immudb (uses testcontainers)
//! STORAGE_BACKEND=immudb cargo test --test interfaces --features immudb
//! ```
//!
//! ## Bus Tests
//! ```bash
//! # Channel (default, in-memory)
//! BUS_BACKEND=channel cargo test --test interfaces --features channel
//!
//! # AMQP/RabbitMQ (uses testcontainers)
//! BUS_BACKEND=amqp cargo test --test interfaces --features amqp
//!
//! # Kafka (uses testcontainers)
//! BUS_BACKEND=kafka cargo test --test interfaces --features kafka
//!
//! # NATS JetStream (uses testcontainers)
//! BUS_BACKEND=nats cargo test --test interfaces --features nats
//!
//! # Google Pub/Sub (uses testcontainers emulator)
//! BUS_BACKEND=pubsub cargo test --test interfaces --features pubsub
//!
//! # AWS SNS/SQS (uses testcontainers/LocalStack)
//! BUS_BACKEND=sns-sqs cargo test --test interfaces --features sns-sqs
//! ```

mod backend;
mod bus_backend;
mod steps;

use cucumber::World;
use steps::dlq::DlqWorld;
use steps::editions::EditionWorld;
use steps::event_bus::EventBusWorld;
use steps::event_query_service::EventQueryServiceWorld;
use steps::event_store::EventStoreWorld;
use steps::event_stream_service::EventStreamServiceWorld;
use steps::notification::NotificationWorld;
use steps::payload_offloading::PayloadWorld;
use steps::position_store::PositionStoreWorld;
use steps::snapshot_store::SnapshotStoreWorld;
use steps::sync_modes::SyncModeWorld;
use steps::upcasting::UpcasterWorld;

#[tokio::main]
async fn main() {
    // Run EventStore tests
    println!("\n=== Running EventStore Interface Tests ===\n");
    EventStoreWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/event_store.feature")
        .await;

    // Run SnapshotStore tests
    println!("\n=== Running SnapshotStore Interface Tests ===\n");
    SnapshotStoreWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/snapshot_store.feature")
        .await;

    // Run PositionStore tests
    println!("\n=== Running PositionStore Interface Tests ===\n");
    PositionStoreWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/position_store.feature")
        .await;

    // Run EventQueryService tests
    println!("\n=== Running EventQueryService Interface Tests ===\n");
    EventQueryServiceWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/event_query_service.feature")
        .await;

    // Run EventStreamService tests
    println!("\n=== Running EventStreamService Interface Tests ===\n");
    EventStreamServiceWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/event_stream_service.feature")
        .await;

    // Run SyncMode tests
    println!("\n=== Running SyncMode Interface Tests ===\n");
    SyncModeWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/sync_modes.feature")
        .await;

    // Run Edition tests
    println!("\n=== Running Edition Interface Tests ===\n");
    EditionWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/editions.feature")
        .await;

    // Run Upcasting tests
    println!("\n=== Running Upcasting Interface Tests ===\n");
    UpcasterWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/upcasting.feature")
        .await;

    // Run Payload Offloading tests
    println!("\n=== Running Payload Offloading Interface Tests ===\n");
    PayloadWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/payload_offloading.feature")
        .await;

    // Run Notification tests
    println!("\n=== Running Notification Interface Tests ===\n");
    NotificationWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/notification.feature")
        .await;

    // Run EventBus tests
    println!("\n=== Running EventBus Interface Tests ===\n");
    EventBusWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/event_bus.feature")
        .await;

    // Run DLQ tests
    println!("\n=== Running DLQ Interface Tests ===\n");
    DlqWorld::cucumber()
        .fail_on_skipped()
        .run("tests/interfaces/features/dlq.feature")
        .await;
}
