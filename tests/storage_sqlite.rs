//! SQLite storage integration tests.
//!
//! Run with: cargo test --test storage_sqlite --features sqlite
//!
//! Uses in-memory database by default, no external dependencies required.

mod storage;

use angzarr::storage::{SqliteEventStore, SqliteSnapshotStore};

/// Get SQLite connection string (in-memory for tests)
fn sqlite_uri() -> String {
    std::env::var("SQLITE_URI").unwrap_or_else(|_| "sqlite::memory:".to_string())
}

/// Clean up test data from SQLite tables
async fn cleanup_test_data(pool: &sqlx::SqlitePool) {
    // Delete all rows with test domains (domains starting with "test_")
    let _ = sqlx::query("DELETE FROM events WHERE domain LIKE 'test_%'")
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM snapshots WHERE domain LIKE 'test_%'")
        .execute(pool)
        .await;
}

#[tokio::test]
async fn test_sqlite_event_store() {
    println!("=== SQLite EventStore Tests ===");
    println!("Connecting to: {}", sqlite_uri());

    let pool = sqlx::SqlitePool::connect(&sqlite_uri())
        .await
        .expect("Failed to connect to SQLite");

    let store = SqliteEventStore::new(pool.clone());
    store.init().await.expect("Failed to initialize schema");

    // Clean up before tests
    cleanup_test_data(&pool).await;

    run_event_store_tests!(&store);

    // Clean up after tests
    cleanup_test_data(&pool).await;

    println!("=== All SQLite EventStore tests PASSED ===");
}

#[tokio::test]
async fn test_sqlite_snapshot_store() {
    println!("=== SQLite SnapshotStore Tests ===");
    println!("Connecting to: {}", sqlite_uri());

    let pool = sqlx::SqlitePool::connect(&sqlite_uri())
        .await
        .expect("Failed to connect to SQLite");

    let store = SqliteSnapshotStore::new(pool.clone());
    store.init().await.expect("Failed to initialize schema");

    // Clean up before tests
    cleanup_test_data(&pool).await;

    run_snapshot_store_tests!(&store);

    // Clean up after tests
    cleanup_test_data(&pool).await;

    println!("=== All SQLite SnapshotStore tests PASSED ===");
}
