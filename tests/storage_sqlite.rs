//! SQLite storage contract tests.
//!
//! Run with: cargo test --test storage_sqlite --features "sqlite test-utils" -- --nocapture
//!
//! These tests verify that SQLite storage implementations correctly fulfill
//! their trait contracts. Uses in-memory SQLite for fast, isolated tests.
//!
//! Note: SQLite stores only the latest snapshot per aggregate, so retention-based
//! historical snapshot tests (test_retention_persist) are skipped.

#![cfg(feature = "sqlite")]

mod storage;

use angzarr::storage::{SqliteEventStore, SqlitePositionStore, SqliteSnapshotStore};
use sqlx::sqlite::SqlitePoolOptions;

/// Create an in-memory SQLite pool with migrations applied.
async fn create_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create SQLite pool");

    sqlx::migrate!("./migrations/sqlite")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

// =============================================================================
// EventStore Tests
// =============================================================================

#[tokio::test]
async fn test_sqlite_event_store() {
    println!("=== SQLite EventStore Tests ===");

    let pool = create_pool().await;
    let store = SqliteEventStore::new(pool);

    run_event_store_tests!(&store);

    println!("=== All SQLite EventStore tests PASSED ===");
}

// =============================================================================
// SnapshotStore Tests
// =============================================================================

/// Run SnapshotStore tests that SQLite supports.
///
/// SQLite stores only the latest snapshot per aggregate (not historical snapshots),
/// so we run the subset of tests that don't require `get_at_seq` to return
/// historical snapshots at specific sequences.
#[tokio::test]
async fn test_sqlite_snapshot_store() {
    use storage::snapshot_store_tests::*;

    println!("=== SQLite SnapshotStore Tests ===");

    let pool = create_pool().await;
    let store = SqliteSnapshotStore::new(pool);

    // Core get tests
    test_get_nonexistent(&store).await;
    println!("  test_get_nonexistent: PASSED");

    test_get_existing(&store).await;
    println!("  test_get_existing: PASSED");

    test_get_preserves_data(&store).await;
    println!("  test_get_preserves_data: PASSED");

    // put tests
    test_put_new(&store).await;
    println!("  test_put_new: PASSED");

    test_put_update(&store).await;
    println!("  test_put_update: PASSED");

    test_put_multiple_updates(&store).await;
    println!("  test_put_multiple_updates: PASSED");

    // delete tests
    test_delete_existing(&store).await;
    println!("  test_delete_existing: PASSED");

    test_delete_nonexistent(&store).await;
    println!("  test_delete_nonexistent: PASSED");

    test_delete_then_recreate(&store).await;
    println!("  test_delete_then_recreate: PASSED");

    // isolation tests
    test_aggregate_isolation(&store).await;
    println!("  test_aggregate_isolation: PASSED");

    test_domain_isolation(&store).await;
    println!("  test_domain_isolation: PASSED");

    // retention tests (subset that works with single-snapshot storage)
    test_retention_transient_cleanup(&store).await;
    println!("  test_retention_transient_cleanup: PASSED");

    // SKIPPED: test_retention_persist - SQLite stores only latest snapshot
    println!("  test_retention_persist: SKIPPED (SQLite stores only latest snapshot)");

    test_retention_default(&store).await;
    println!("  test_retention_default: PASSED");

    // edition tests (use get() not get_at_seq())
    test_edition_isolation(&store).await;
    println!("  test_edition_isolation: PASSED");

    test_edition_delete_independence(&store).await;
    println!("  test_edition_delete_independence: PASSED");

    // large state tests
    test_large_state_100kb(&store).await;
    println!("  test_large_state_100kb: PASSED");

    println!("=== All SQLite SnapshotStore tests PASSED ===");
}

// =============================================================================
// PositionStore Tests
// =============================================================================

#[tokio::test]
async fn test_sqlite_position_store() {
    println!("=== SQLite PositionStore Tests ===");

    let pool = create_pool().await;
    let store = SqlitePositionStore::new(pool);

    run_position_store_tests!(&store);

    println!("=== All SQLite PositionStore tests PASSED ===");
}
