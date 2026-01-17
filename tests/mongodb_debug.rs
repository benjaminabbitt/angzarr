//! MongoDB debugging test - run with:
//! cargo test --test mongodb_debug --features mongodb -- --nocapture

use std::sync::Arc;

use prost_types::Any;
use uuid::Uuid;

use angzarr::interfaces::EventStore;
use angzarr::proto::{event_page, EventPage};
use angzarr::storage::MongoEventStore;

/// Get MongoDB connection string from environment or default to NodePort
fn mongodb_uri() -> String {
    std::env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:30017".to_string())
}

fn make_test_event(seq: u32, event_type: &str) -> EventPage {
    EventPage {
        sequence: Some(event_page::Sequence::Num(seq)),
        created_at: None,
        event: Some(Any {
            type_url: format!("type.example/{}", event_type),
            value: vec![1, 2, 3, seq as u8],
        }),
        synchronous: false,
    }
}

#[tokio::test]
async fn test_mongodb_roundtrip() {
    println!("Connecting to MongoDB at: {}", mongodb_uri());

    let client = mongodb::Client::with_uri_str(&mongodb_uri())
        .await
        .expect("Failed to connect to MongoDB");

    println!("Connected to MongoDB");

    // Use a test database
    let database_name = "test_angzarr";
    let event_store = Arc::new(
        MongoEventStore::new(&client, database_name)
            .await
            .expect("Failed to create event store"),
    );

    println!("Event store created");

    // Create a unique domain/root for this test
    let domain = "test_domain";
    let root = Uuid::new_v4();

    println!("Testing with domain={}, root={}", domain, root);

    // Add events
    let events = vec![
        make_test_event(0, "TestCreated"),
        make_test_event(1, "TestUpdated"),
    ];

    println!("Adding {} events...", events.len());
    event_store
        .add(domain, root, events)
        .await
        .expect("Failed to add events");
    println!("Events added successfully");

    // Read them back
    println!("Reading events back...");
    let retrieved = event_store
        .get(domain, root)
        .await
        .expect("Failed to get events");

    println!("Retrieved {} events", retrieved.len());

    for (i, event) in retrieved.iter().enumerate() {
        println!(
            "  Event {}: sequence={:?}, type={:?}",
            i,
            event.sequence,
            event.event.as_ref().map(|e| &e.type_url)
        );
    }

    assert_eq!(
        retrieved.len(),
        2,
        "Expected 2 events, got {}",
        retrieved.len()
    );

    // Also test get_from
    println!("Testing get_from(0)...");
    let from_zero = event_store
        .get_from(domain, root, 0)
        .await
        .expect("Failed to get_from");
    println!("get_from(0) returned {} events", from_zero.len());
    assert_eq!(from_zero.len(), 2);

    println!("Testing get_from(1)...");
    let from_one = event_store
        .get_from(domain, root, 1)
        .await
        .expect("Failed to get_from(1)");
    println!("get_from(1) returned {} events", from_one.len());
    assert_eq!(from_one.len(), 1);

    // Clean up
    client.database(database_name).drop().await.ok();
    println!("Test completed successfully!");
}

#[tokio::test]
async fn test_mongodb_read_existing_data() {
    // This test reads from the deployed angzarr database to see if we can read existing events
    println!("Connecting to MongoDB at: {}", mongodb_uri());

    let client = mongodb::Client::with_uri_str(&mongodb_uri())
        .await
        .expect("Failed to connect to MongoDB");

    println!("Connected to MongoDB");

    // Use the actual angzarr database
    let database_name = "angzarr";
    let event_store = Arc::new(
        MongoEventStore::new(&client, database_name)
            .await
            .expect("Failed to create event store"),
    );

    println!("Event store created for production database");

    // List all domains
    let domains = event_store
        .list_domains()
        .await
        .expect("Failed to list domains");
    println!("Domains in database: {:?}", domains);

    // For each domain, list a few roots and try to read events
    for domain in &domains {
        let roots = event_store
            .list_roots(domain)
            .await
            .expect("Failed to list roots");

        println!("Domain '{}' has {} roots", domain, roots.len());

        // Test first root
        if let Some(root) = roots.first() {
            println!("Testing read for domain={}, root={}", domain, root);

            let events = event_store
                .get(domain, *root)
                .await
                .expect("Failed to get events");

            println!("  Retrieved {} events", events.len());

            for (i, event) in events.iter().enumerate() {
                println!(
                    "    Event {}: sequence={:?}, type={:?}",
                    i,
                    event.sequence,
                    event.event.as_ref().map(|e| &e.type_url)
                );
            }
        }
    }
}
