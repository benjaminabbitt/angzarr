//! Interface tests for storage backends using Cucumber.
//!
//! These tests verify that all storage implementations conform to the same contract.
//! Select a backend via environment variable:
//!
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

mod backend;
mod steps;

use cucumber::World;
use steps::editions::EditionWorld;
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
}
