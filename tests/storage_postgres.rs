//! PostgreSQL storage integration tests.
//!
//! Run with: cargo test --test storage_postgres --features postgres -- --nocapture
//!
//! Requires: DATABASE_URL or POSTGRES_URI env var, or PostgreSQL on localhost:5432

mod storage;

use angzarr::storage::{PostgresEventStore, PostgresPositionStore, PostgresSnapshotStore};

fn postgres_uri() -> String {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("POSTGRES_URI"))
        .unwrap_or_else(|_| "postgres://angzarr:angzarr@localhost:5432/angzarr".to_string())
}

async fn cleanup_events(pool: &sqlx::PgPool) {
    let _ = sqlx::query("DELETE FROM events WHERE domain LIKE 'test_%'")
        .execute(pool)
        .await;
}

async fn cleanup_snapshots(pool: &sqlx::PgPool) {
    let _ = sqlx::query("DELETE FROM snapshots WHERE domain LIKE 'test_%'")
        .execute(pool)
        .await;
}

async fn cleanup_positions(pool: &sqlx::PgPool) {
    let _ = sqlx::query("DELETE FROM positions WHERE handler LIKE 'test_%'")
        .execute(pool)
        .await;
}

async fn connect_and_migrate() -> sqlx::PgPool {
    let pool = sqlx::PgPool::connect(&postgres_uri())
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
    println!("Connecting to: {}", postgres_uri());

    let pool = connect_and_migrate().await;
    let store = PostgresEventStore::new(pool.clone());

    cleanup_events(&pool).await;
    run_event_store_tests!(&store);
    cleanup_events(&pool).await;

    println!("=== All PostgreSQL EventStore tests PASSED ===");
}

#[tokio::test]
async fn test_postgres_snapshot_store() {
    println!("=== PostgreSQL SnapshotStore Tests ===");
    println!("Connecting to: {}", postgres_uri());

    let pool = connect_and_migrate().await;
    let store = PostgresSnapshotStore::new(pool.clone());

    cleanup_snapshots(&pool).await;
    run_snapshot_store_tests!(&store);
    cleanup_snapshots(&pool).await;

    println!("=== All PostgreSQL SnapshotStore tests PASSED ===");
}

#[tokio::test]
async fn test_postgres_position_store() {
    println!("=== PostgreSQL PositionStore Tests ===");
    println!("Connecting to: {}", postgres_uri());

    let pool = connect_and_migrate().await;
    let store = PostgresPositionStore::new(pool.clone());

    cleanup_positions(&pool).await;
    run_position_store_tests!(&store);
    cleanup_positions(&pool).await;

    println!("=== All PostgreSQL PositionStore tests PASSED ===");
}
