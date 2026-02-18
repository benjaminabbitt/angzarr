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
use steps::event_store::EventStoreWorld;
use steps::position_store::PositionStoreWorld;
use steps::snapshot_store::SnapshotStoreWorld;

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
}
