//! PostgreSQL storage integration tests.
//!
//! Run with: cargo test --test storage_postgres --features postgres -- --ignored --nocapture
//!
//! Requires: DATABASE_URL or POSTGRES_URI env var, or PostgreSQL on localhost:5432

mod storage;

use angzarr::storage::{PostgresEventStore, PostgresSnapshotStore};

fn postgres_uri() -> String {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("POSTGRES_URI"))
        .unwrap_or_else(|_| "postgres://angzarr:angzarr@localhost:5432/angzarr".to_string())
}

/// Clean up test data from PostgreSQL tables
async fn cleanup_test_data(pool: &sqlx::PgPool) {
    // Delete all rows with test domains (domains starting with "test_")
    let _ = sqlx::query("DELETE FROM events WHERE domain LIKE 'test_%'")
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM snapshots WHERE domain LIKE 'test_%'")
        .execute(pool)
        .await;
}

#[tokio::test]
#[ignore = "requires running PostgreSQL instance"]
async fn test_postgres_event_store() {
    println!("=== PostgreSQL EventStore Tests ===");
    println!("Connecting to: {}", postgres_uri());

    let pool = sqlx::PgPool::connect(&postgres_uri())
        .await
        .expect("Failed to connect to PostgreSQL");

    let store = PostgresEventStore::new(pool.clone());
    store.init().await.expect("Failed to initialize schema");

    // Clean up before tests
    cleanup_test_data(&pool).await;

    run_event_store_tests!(&store);

    // Clean up after tests
    cleanup_test_data(&pool).await;

    println!("=== All PostgreSQL EventStore tests PASSED ===");
}

#[tokio::test]
#[ignore = "requires running PostgreSQL instance"]
async fn test_postgres_snapshot_store() {
    println!("=== PostgreSQL SnapshotStore Tests ===");
    println!("Connecting to: {}", postgres_uri());

    let pool = sqlx::PgPool::connect(&postgres_uri())
        .await
        .expect("Failed to connect to PostgreSQL");

    let store = PostgresSnapshotStore::new(pool.clone());
    store.init().await.expect("Failed to initialize schema");

    // Clean up before tests
    cleanup_test_data(&pool).await;

    run_snapshot_store_tests!(&store);

    // Clean up after tests
    cleanup_test_data(&pool).await;

    println!("=== All PostgreSQL SnapshotStore tests PASSED ===");
}
