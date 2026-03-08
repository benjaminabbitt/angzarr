//! PostgreSQL storage contract tests using testcontainers.
//!
//! Run with: cargo test --test storage_postgres --features "postgres test-utils" -- --nocapture
//!
//! These tests verify that PostgreSQL storage implementations correctly fulfill
//! their trait contracts. Uses testcontainers-rs to spin up PostgreSQL.

#![cfg(feature = "postgres")]

mod storage;

use std::time::Duration;

use angzarr::storage::{PostgresEventStore, PostgresPositionStore, PostgresSnapshotStore};
use sqlx::postgres::PgPoolOptions;
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};

/// Start PostgreSQL container.
///
/// Returns (container, pool) where pool is ready to use.
async fn start_postgres() -> (testcontainers::ContainerAsync<GenericImage>, sqlx::PgPool) {
    let image = GenericImage::new("postgres", "16")
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stdout(
            "database system is ready to accept connections",
        ));

    let container = image
        .with_env_var("POSTGRES_USER", "testuser")
        .with_env_var("POSTGRES_PASSWORD", "testpass")
        .with_env_var("POSTGRES_DB", "testdb")
        .with_startup_timeout(Duration::from_secs(60))
        .start()
        .await
        .expect("Failed to start Postgres container");

    // Brief delay for full readiness
    tokio::time::sleep(Duration::from_secs(2)).await;

    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");

    let host = container.get_host().await.expect("Failed to get host");

    let connection_string = format!("postgres://testuser:testpass@{}:{}/testdb", host, host_port);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&connection_string)
        .await
        .expect("Failed to connect to PostgreSQL");

    // Run migrations
    sqlx::migrate!("./migrations/postgres")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    println!("PostgreSQL available at: {}", connection_string);

    (container, pool)
}

// =============================================================================
// EventStore Tests
// =============================================================================

#[tokio::test]
async fn test_postgres_event_store() {
    println!("=== PostgreSQL EventStore Tests ===");
    println!("Starting PostgreSQL container...");

    let (_container, pool) = start_postgres().await;
    let store = PostgresEventStore::new(pool);

    println!("Running EventStore tests...");
    run_event_store_tests!(&store);

    println!("=== All PostgreSQL EventStore tests PASSED ===");
}

// =============================================================================
// SnapshotStore Tests
// =============================================================================

#[tokio::test]
async fn test_postgres_snapshot_store() {
    println!("=== PostgreSQL SnapshotStore Tests ===");
    println!("Starting PostgreSQL container...");

    let (_container, pool) = start_postgres().await;
    let store = PostgresSnapshotStore::new(pool);

    println!("Running SnapshotStore tests...");
    run_snapshot_store_tests!(&store);

    println!("=== All PostgreSQL SnapshotStore tests PASSED ===");
}

// =============================================================================
// PositionStore Tests
// =============================================================================

#[tokio::test]
async fn test_postgres_position_store() {
    println!("=== PostgreSQL PositionStore Tests ===");
    println!("Starting PostgreSQL container...");

    let (_container, pool) = start_postgres().await;
    let store = PostgresPositionStore::new(pool);

    println!("Running PositionStore tests...");
    run_position_store_tests!(&store);

    println!("=== All PostgreSQL PositionStore tests PASSED ===");
}
