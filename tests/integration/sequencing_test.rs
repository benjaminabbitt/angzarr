//! Sequencing integration tests.
//!
//! Tests event sequencing, ordering, and sequence number behavior.

#[path = "../common/mod.rs"]
mod common;

use common::{
    build_command_book, build_command_book_at_sequence, build_query, create_gateway_client,
    create_query_client, extract_event_type, extract_sequence,
};
use uuid::Uuid;

// Examples proto types
#[allow(dead_code)]
mod examples_proto {
    include!(concat!(env!("OUT_DIR"), "/examples.rs"));
}

use examples_proto::{AddLoyaltyPoints, CreateCustomer};

/// Tests that the first command on a new aggregate produces sequence 0.
#[tokio::test]
async fn test_first_event_has_sequence_zero() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer (first command)
    let command = CreateCustomer {
        name: "Sequence Zero Test".to_string(),
        email: "seq0@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok(), "Command execution failed");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query events and verify sequence
    let query = build_query("customer", customer_id);
    let query_response = query_client.get_event_book(query).await;
    assert!(query_response.is_ok());

    let event_book = query_response.unwrap().into_inner();
    assert_eq!(event_book.pages.len(), 1, "Expected exactly 1 event");

    let first_event = &event_book.pages[0];
    let sequence = extract_sequence(first_event);
    assert_eq!(sequence, 0, "First event should have sequence 0");
}

/// Tests that multiple sequential commands produce incrementing sequence numbers.
#[tokio::test]
async fn test_sequential_commands_increment_sequence() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // First command: Create customer
    let create_command = CreateCustomer {
        name: "Sequential Test".to_string(),
        email: "sequential@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, create_command, "examples.CreateCustomer");

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok(), "First command failed");

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Second command: Add loyalty points (at sequence 1)
    let add_points_command = AddLoyaltyPoints {
        points: 100,
        reason: "welcome".to_string(),
    };
    let command_book = build_command_book_at_sequence(
        "customer",
        customer_id,
        add_points_command,
        "examples.AddLoyaltyPoints",
        1,
    );

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok(), "Second command failed");

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Third command: Add more loyalty points (at sequence 2)
    let add_points_command2 = AddLoyaltyPoints {
        points: 50,
        reason: "bonus".to_string(),
    };
    let command_book = build_command_book_at_sequence(
        "customer",
        customer_id,
        add_points_command2,
        "examples.AddLoyaltyPoints",
        2,
    );

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok(), "Third command failed");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query all events and verify sequence numbers
    let query = build_query("customer", customer_id);
    let query_response = query_client.get_event_book(query).await;
    assert!(query_response.is_ok());

    let event_book = query_response.unwrap().into_inner();
    assert_eq!(event_book.pages.len(), 3, "Expected exactly 3 events");

    // Verify sequences are 0, 1, 2
    for (i, page) in event_book.pages.iter().enumerate() {
        let sequence = extract_sequence(page);
        assert_eq!(
            sequence, i as u32,
            "Event {} should have sequence {}, got {}",
            i, i, sequence
        );
    }
}

/// Tests that events are returned in sequence order.
#[tokio::test]
async fn test_events_returned_in_sequence_order() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create 5 events
    let create_command = CreateCustomer {
        name: "Order Test".to_string(),
        email: "order@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, create_command, "examples.CreateCustomer");
    gateway_client.execute(command_book).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    for i in 1..5 {
        let add_points = AddLoyaltyPoints {
            points: i * 10,
            reason: format!("batch-{}", i),
        };
        let command_book = build_command_book_at_sequence(
            "customer",
            customer_id,
            add_points,
            "examples.AddLoyaltyPoints",
            i as u32,
        );
        gateway_client.execute(command_book).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query and verify ordering
    let query = build_query("customer", customer_id);
    let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

    assert_eq!(event_book.pages.len(), 5, "Expected 5 events");

    let mut prev_sequence = None;
    for page in &event_book.pages {
        let current_sequence = extract_sequence(page);
        if let Some(prev) = prev_sequence {
            assert!(
                current_sequence > prev,
                "Events not in ascending order: {} should be > {}",
                current_sequence,
                prev
            );
        }
        prev_sequence = Some(current_sequence);
    }
}

/// Tests that a command with wrong sequence number is rejected.
#[tokio::test]
async fn test_wrong_sequence_rejected() {
    let mut gateway_client = create_gateway_client().await;
    let customer_id = Uuid::new_v4();

    // First command: Create customer
    let create_command = CreateCustomer {
        name: "Wrong Seq Test".to_string(),
        email: "wrongseq@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, create_command, "examples.CreateCustomer");

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok(), "First command should succeed");

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Second command with wrong sequence (expecting 1, sending 5)
    let add_points = AddLoyaltyPoints {
        points: 100,
        reason: "wrong_seq".to_string(),
    };
    let command_book = build_command_book_at_sequence(
        "customer",
        customer_id,
        add_points,
        "examples.AddLoyaltyPoints",
        5, // Wrong sequence - aggregate is at sequence 1
    );

    let response = gateway_client.execute(command_book).await;
    assert!(
        response.is_err(),
        "Command with wrong sequence should be rejected"
    );

    let status = response.unwrap_err();
    // Check it's a precondition failure or aborted
    assert!(
        status.code() == tonic::Code::FailedPrecondition
            || status.code() == tonic::Code::Aborted
            || status.message().to_lowercase().contains("sequence"),
        "Error should indicate sequence mismatch: {:?}",
        status
    );
}

/// Tests that response includes the correct sequence in the returned events.
#[tokio::test]
async fn test_response_includes_event_sequence() {
    let mut gateway_client = create_gateway_client().await;
    let customer_id = Uuid::new_v4();

    let command = CreateCustomer {
        name: "Response Seq Test".to_string(),
        email: "response@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok());

    let response = response.unwrap().into_inner();
    let events = response.events.expect("Expected events in response");

    assert!(!events.pages.is_empty(), "Expected at least one event");

    let first_page = &events.pages[0];
    let sequence = extract_sequence(first_page);
    assert_eq!(sequence, 0, "Response event should have sequence 0");
}

/// Tests querying events with specific bounds returns correct sequence range.
#[tokio::test]
async fn test_query_bounds_respect_sequence() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create 5 events
    let create_command = CreateCustomer {
        name: "Bounds Test".to_string(),
        email: "bounds@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, create_command, "examples.CreateCustomer");
    gateway_client.execute(command_book).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    for i in 1..5 {
        let add_points = AddLoyaltyPoints {
            points: i * 10,
            reason: format!("bounds-{}", i),
        };
        let command_book = build_command_book_at_sequence(
            "customer",
            customer_id,
            add_points,
            "examples.AddLoyaltyPoints",
            i as u32,
        );
        gateway_client.execute(command_book).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query only events 1-3
    let query = common::Query {
        domain: "customer".to_string(),
        root: Some(common::ProtoUuid {
            value: customer_id.as_bytes().to_vec(),
        }),
        lower_bound: 1,
        upper_bound: 3,
    };

    let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

    // Should get events with sequences 1, 2, 3
    assert_eq!(event_book.pages.len(), 3, "Expected 3 events for bounds [1,3]");

    for page in &event_book.pages {
        let seq = extract_sequence(page);
        assert!(
            seq >= 1 && seq <= 3,
            "Event sequence {} outside bounds [1,3]",
            seq
        );
    }
}

/// Tests that events from different aggregates have independent sequences.
#[tokio::test]
async fn test_independent_aggregate_sequences() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;

    let customer_id_1 = Uuid::new_v4();
    let customer_id_2 = Uuid::new_v4();

    // Create first customer with 3 events
    let create1 = CreateCustomer {
        name: "Customer One".to_string(),
        email: "one@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id_1, create1, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    let add1 = AddLoyaltyPoints { points: 100, reason: "c1-1".to_string() };
    let cmd = build_command_book_at_sequence("customer", customer_id_1, add1, "examples.AddLoyaltyPoints", 1);
    gateway_client.execute(cmd).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    let add2 = AddLoyaltyPoints { points: 200, reason: "c1-2".to_string() };
    let cmd = build_command_book_at_sequence("customer", customer_id_1, add2, "examples.AddLoyaltyPoints", 2);
    gateway_client.execute(cmd).await.unwrap();

    // Create second customer with only 1 event
    let create2 = CreateCustomer {
        name: "Customer Two".to_string(),
        email: "two@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id_2, create2, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query both customers
    let query1 = build_query("customer", customer_id_1);
    let events1 = query_client.get_event_book(query1).await.unwrap().into_inner();
    assert_eq!(events1.pages.len(), 3, "Customer 1 should have 3 events");

    let query2 = build_query("customer", customer_id_2);
    let events2 = query_client.get_event_book(query2).await.unwrap().into_inner();
    assert_eq!(events2.pages.len(), 1, "Customer 2 should have 1 event");

    // Both should start at sequence 0
    assert_eq!(extract_sequence(&events1.pages[0]), 0);
    assert_eq!(extract_sequence(&events2.pages[0]), 0);
}

/// Tests that event types are correctly preserved in sequence.
#[tokio::test]
async fn test_event_types_in_sequence() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer
    let create = CreateCustomer {
        name: "Event Type Test".to_string(),
        email: "eventtype@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    // Add points
    let add = AddLoyaltyPoints { points: 100, reason: "test".to_string() };
    let cmd = build_command_book_at_sequence("customer", customer_id, add, "examples.AddLoyaltyPoints", 1);
    gateway_client.execute(cmd).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query and verify event types match sequence
    let query = build_query("customer", customer_id);
    let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

    assert_eq!(event_book.pages.len(), 2);

    // Sequence 0 should be CustomerCreated
    let event0 = event_book.pages[0].event.as_ref().unwrap();
    let type0 = extract_event_type(event0);
    assert!(type0.contains("CustomerCreated"), "Seq 0 should be CustomerCreated, got {}", type0);
    assert_eq!(extract_sequence(&event_book.pages[0]), 0);

    // Sequence 1 should be LoyaltyPointsAdded
    let event1 = event_book.pages[1].event.as_ref().unwrap();
    let type1 = extract_event_type(event1);
    assert!(type1.contains("LoyaltyPointsAdded"), "Seq 1 should be LoyaltyPointsAdded, got {}", type1);
    assert_eq!(extract_sequence(&event_book.pages[1]), 1);
}
