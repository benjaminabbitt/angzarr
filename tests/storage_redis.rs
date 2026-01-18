//! Redis storage integration tests.
//!
//! Run with: cargo test --test storage_redis --features redis -- --ignored --nocapture
//!
//! Requires: REDIS_URI env var or Redis on localhost:6379
//!
//! Note: Tests use unique key prefixes to avoid data conflicts between runs.

mod storage;

use angzarr::storage::redis::{RedisEventStore, RedisSnapshotStore};

fn redis_uri() -> String {
    std::env::var("REDIS_URI").unwrap_or_else(|_| "redis://localhost:6379".to_string())
}

fn test_prefix() -> String {
    format!(
        "test_{}",
        uuid::Uuid::new_v4().to_string().replace("-", "")[..8].to_string()
    )
}

#[tokio::test]
#[ignore = "requires running Redis instance"]
async fn test_redis_event_store() {
    println!("=== Redis EventStore Tests ===");
    println!("Connecting to: {}", redis_uri());

    let prefix = test_prefix();
    println!("Using test prefix: {}", prefix);

    let store = RedisEventStore::new(&redis_uri(), Some(&prefix))
        .await
        .expect("Failed to connect to Redis");

    run_event_store_tests!(&store);

    println!("=== All Redis EventStore tests PASSED ===");
}

#[tokio::test]
#[ignore = "requires running Redis instance"]
async fn test_redis_snapshot_store() {
    println!("=== Redis SnapshotStore Tests ===");
    println!("Connecting to: {}", redis_uri());

    let prefix = test_prefix();
    println!("Using test prefix: {}", prefix);

    let store = RedisSnapshotStore::new(&redis_uri(), Some(&prefix))
        .await
        .expect("Failed to connect to Redis");

    run_snapshot_store_tests!(&store);

    println!("=== All Redis SnapshotStore tests PASSED ===");
}
