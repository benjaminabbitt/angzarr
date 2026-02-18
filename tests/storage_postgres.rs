//! PostgreSQL storage integration tests using testcontainers.
//!
//! Run with: cargo test --test storage_postgres --features postgres -- --nocapture
//!
//! These tests spin up PostgreSQL in a container using testcontainers-rs,
//! run migrations, and test all storage interfaces.

mod storage;

use std::time::Duration;

use angzarr::storage::{PostgresEventStore, PostgresPositionStore, PostgresSnapshotStore};
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};

/// Start PostgreSQL container.
///
/// Returns (container, connection_string) where connection_string is suitable
/// for sqlx PgPool connection.
async fn start_postgres() -> (testcontainers::ContainerAsync<GenericImage>, String) {
    // PostgreSQL prints "database system is ready to accept connections" twice:
    // once during initial setup and once when fully ready.
    // We wait for the message but add a small delay to ensure full readiness.
    let image = GenericImage::new("postgres", "16")
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stdout(
            "database system is ready to accept connections",
        ));

    let container = image
        .with_env_var("POSTGRES_USER", "angzarr")
        .with_env_var("POSTGRES_PASSWORD", "angzarr")
        .with_env_var("POSTGRES_DB", "angzarr")
        .with_startup_timeout(Duration::from_secs(60))
        .start()
        .await
        .expect("Failed to start postgres container");

    // Brief delay to ensure PostgreSQL is fully ready to accept connections
    tokio::time::sleep(Duration::from_secs(1)).await;

    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get mapped port");

    let host = container
        .get_host()
        .await
        .expect("Failed to get container host");

    let connection_string = format!("postgres://angzarr:angzarr@{}:{}/angzarr", host, host_port);

    println!("PostgreSQL available at: {}", connection_string);

    (container, connection_string)
}

/// Connect to PostgreSQL and run migrations.
async fn connect_and_migrate(connection_string: &str) -> sqlx::PgPool {
    let pool = sqlx::PgPool::connect(connection_string)
        .await
        .expect("Failed to connect to PostgreSQL");

    sqlx::migrate!("migrations/postgres")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

#[tokio::test]
async fn test_postgres_event_store() {
    println!("=== PostgreSQL EventStore Tests ===");
    println!("Starting PostgreSQL container...");

    let (_container, connection_string) = start_postgres().await;
    let pool = connect_and_migrate(&connection_string).await;
    let store = PostgresEventStore::new(pool);

    println!("Running EventStore tests...");
    run_event_store_tests!(&store);

    println!("=== All PostgreSQL EventStore tests PASSED ===");
    // Container is dropped here, stopping PostgreSQL
}

#[tokio::test]
async fn test_postgres_snapshot_store() {
    println!("=== PostgreSQL SnapshotStore Tests ===");
    println!("Starting PostgreSQL container...");

    let (_container, connection_string) = start_postgres().await;
    let pool = connect_and_migrate(&connection_string).await;
    let store = PostgresSnapshotStore::new(pool);

    println!("Running SnapshotStore tests...");
    run_snapshot_store_tests!(&store);

    println!("=== All PostgreSQL SnapshotStore tests PASSED ===");
}

#[tokio::test]
async fn test_postgres_position_store() {
    println!("=== PostgreSQL PositionStore Tests ===");
    println!("Starting PostgreSQL container...");

    let (_container, connection_string) = start_postgres().await;
    let pool = connect_and_migrate(&connection_string).await;
    let store = PostgresPositionStore::new(pool);

    println!("Running PositionStore tests...");
    run_position_store_tests!(&store);

    println!("=== All PostgreSQL PositionStore tests PASSED ===");
}
