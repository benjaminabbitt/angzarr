//! SQLite storage integration tests.
//!
//! Run with: cargo test --test storage_sqlite --features sqlite
//!
//! Uses in-memory database by default, no external dependencies required.

mod storage;

use angzarr::storage::{SqliteEventStore, SqlitePositionStore, SqliteSnapshotStore};

/// Get SQLite connection string (in-memory for tests)
fn sqlite_uri() -> String {
    std::env::var("SQLITE_URI").unwrap_or_else(|_| "sqlite::memory:".to_string())
}

async fn cleanup_events(pool: &sqlx::SqlitePool) {
    let _ = sqlx::query("DELETE FROM events WHERE domain LIKE 'test_%'")
        .execute(pool)
        .await;
}

async fn cleanup_snapshots(pool: &sqlx::SqlitePool) {
    let _ = sqlx::query("DELETE FROM snapshots WHERE domain LIKE 'test_%'")
        .execute(pool)
        .await;
}

async fn cleanup_positions(pool: &sqlx::SqlitePool) {
    let _ = sqlx::query("DELETE FROM positions WHERE handler LIKE 'test_%'")
        .execute(pool)
        .await;
}

async fn connect_and_migrate() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect(&sqlite_uri())
        .await
        .expect("Failed to connect to SQLite");

    sqlx::migrate!("migrations/sqlite")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

#[tokio::test]
async fn test_sqlite_event_store() {
    println!("=== SQLite EventStore Tests ===");
    println!("Connecting to: {}", sqlite_uri());

    let pool = connect_and_migrate().await;
    let store = SqliteEventStore::new(pool.clone());

    cleanup_events(&pool).await;
    run_event_store_tests!(&store);
    cleanup_events(&pool).await;

    println!("=== All SQLite EventStore tests PASSED ===");
}

#[tokio::test]
async fn test_sqlite_snapshot_store() {
    println!("=== SQLite SnapshotStore Tests ===");
    println!("Connecting to: {}", sqlite_uri());

    let pool = connect_and_migrate().await;
    let store = SqliteSnapshotStore::new(pool.clone());

    cleanup_snapshots(&pool).await;
    run_snapshot_store_tests!(&store);
    cleanup_snapshots(&pool).await;

    println!("=== All SQLite SnapshotStore tests PASSED ===");
}

#[tokio::test]
async fn test_sqlite_position_store() {
    println!("=== SQLite PositionStore Tests ===");
    println!("Connecting to: {}", sqlite_uri());

    let pool = connect_and_migrate().await;
    let store = SqlitePositionStore::new(pool.clone());

    cleanup_positions(&pool).await;
    run_position_store_tests!(&store);
    cleanup_positions(&pool).await;

    println!("=== All SQLite PositionStore tests PASSED ===");
}
