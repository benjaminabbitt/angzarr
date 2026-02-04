//! MongoDB storage integration tests.
//!
//! Run with: cargo test --test storage_mongodb --features mongodb -- --ignored --nocapture
//!
//! Requires: MONGODB_URI env var or MongoDB on localhost:27017

mod storage;

use angzarr::storage::MongoEventStore;
use angzarr::storage::MongoPositionStore;
use angzarr::storage::MongoSnapshotStore;

fn mongodb_uri() -> String {
    std::env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string())
}

fn mongodb_database() -> String {
    std::env::var("MONGODB_DATABASE").unwrap_or_else(|_| "angzarr".to_string())
}

/// Clean up test data from a specific collection only.
/// Each test cleans only its own collection to avoid interference when running in parallel.
async fn cleanup_collection(client: &mongodb::Client, db_name: &str, collection: &str, field: &str) {
    let db = client.database(db_name);
    let coll = db.collection::<mongodb::bson::Document>(collection);
    let _ = coll
        .delete_many(mongodb::bson::doc! { field: { "$regex": "^test_" } })
        .await;
}

#[tokio::test]
#[ignore = "requires running MongoDB instance"]
async fn test_mongodb_event_store() {
    println!("=== MongoDB EventStore Tests ===");
    println!("Connecting to: {}", mongodb_uri());

    let client = mongodb::Client::with_uri_str(&mongodb_uri())
        .await
        .expect("Failed to connect to MongoDB");

    let db_name = mongodb_database();
    println!("Using database: {}", db_name);

    // Clean up only events collection before tests (isolated from other store tests)
    cleanup_collection(&client, &db_name, "events", "domain").await;

    let store = MongoEventStore::new(&client, &db_name)
        .await
        .expect("Failed to create event store");

    run_event_store_tests!(&store);

    // Clean up only events collection after tests
    cleanup_collection(&client, &db_name, "events", "domain").await;

    println!("=== All MongoDB EventStore tests PASSED ===");
}

#[tokio::test]
#[ignore = "requires running MongoDB instance"]
async fn test_mongodb_snapshot_store() {
    println!("=== MongoDB SnapshotStore Tests ===");
    println!("Connecting to: {}", mongodb_uri());

    let client = mongodb::Client::with_uri_str(&mongodb_uri())
        .await
        .expect("Failed to connect to MongoDB");

    let db_name = mongodb_database();
    println!("Using database: {}", db_name);

    // Clean up only snapshots collection before tests (isolated from other store tests)
    cleanup_collection(&client, &db_name, "snapshots", "domain").await;

    let store = MongoSnapshotStore::new(&client, &db_name)
        .await
        .expect("Failed to create snapshot store");

    run_snapshot_store_tests!(&store);

    // Clean up only snapshots collection after tests
    cleanup_collection(&client, &db_name, "snapshots", "domain").await;

    println!("=== All MongoDB SnapshotStore tests PASSED ===");
}

#[tokio::test]
#[ignore = "requires running MongoDB instance"]
async fn test_mongodb_position_store() {
    println!("=== MongoDB PositionStore Tests ===");
    println!("Connecting to: {}", mongodb_uri());

    let client = mongodb::Client::with_uri_str(&mongodb_uri())
        .await
        .expect("Failed to connect to MongoDB");

    let db_name = mongodb_database();
    println!("Using database: {}", db_name);

    // Clean up only positions collection before tests (isolated from other store tests)
    cleanup_collection(&client, &db_name, "positions", "handler").await;

    let store = MongoPositionStore::new(&client, &db_name)
        .await
        .expect("Failed to create position store");

    run_position_store_tests!(&store);

    // Clean up only positions collection after tests
    cleanup_collection(&client, &db_name, "positions", "handler").await;

    println!("=== All MongoDB PositionStore tests PASSED ===");
}
