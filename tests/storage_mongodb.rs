//! MongoDB storage integration tests.
//!
//! Run with: cargo test --test storage_mongodb --features mongodb -- --ignored --nocapture
//!
//! Requires: MONGODB_URI env var or MongoDB on localhost:27017

mod storage;

use angzarr::storage::MongoEventStore;
use angzarr::storage::MongoSnapshotStore;

fn mongodb_uri() -> String {
    std::env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string())
}

fn mongodb_database() -> String {
    std::env::var("MONGODB_DATABASE").unwrap_or_else(|_| "angzarr".to_string())
}

/// Clean up test data from MongoDB collections
async fn cleanup_test_data(client: &mongodb::Client, db_name: &str) {
    let db = client.database(db_name);

    // Delete all documents with test domains (domains starting with "test_")
    let events = db.collection::<mongodb::bson::Document>("events");
    let _ = events
        .delete_many(mongodb::bson::doc! { "domain": { "$regex": "^test_" } })
        .await;

    let snapshots = db.collection::<mongodb::bson::Document>("snapshots");
    let _ = snapshots
        .delete_many(mongodb::bson::doc! { "domain": { "$regex": "^test_" } })
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

    // Clean up before tests
    cleanup_test_data(&client, &db_name).await;

    let store = MongoEventStore::new(&client, &db_name)
        .await
        .expect("Failed to create event store");

    run_event_store_tests!(&store);

    // Clean up after tests
    cleanup_test_data(&client, &db_name).await;

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

    // Clean up before tests
    cleanup_test_data(&client, &db_name).await;

    let store = MongoSnapshotStore::new(&client, &db_name)
        .await
        .expect("Failed to create snapshot store");

    run_snapshot_store_tests!(&store);

    // Clean up after tests
    cleanup_test_data(&client, &db_name).await;

    println!("=== All MongoDB SnapshotStore tests PASSED ===");
}
