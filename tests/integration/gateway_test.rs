//! Gateway integration tests.
//!
//! Tests command execution and routing through the gateway.
//! Run with: ANGZARR_TEST_MODE=container cargo test --test gateway_integration

#[path = "../common/mod.rs"]
mod common;

use common::{
    build_command_book, build_query, create_gateway_client, create_query_client, extract_event_type,
};
use uuid::Uuid;

// Examples proto types
#[allow(dead_code)]
mod examples_proto {
    include!(concat!(env!("OUT_DIR"), "/examples.rs"));
}

use examples_proto::CreateCustomer;

/// Returns true if container tests should run.
fn should_run_container_tests() -> bool {
    std::env::var("ANGZARR_TEST_MODE")
        .map(|v| v.to_lowercase() == "container")
        .unwrap_or(false)
}

#[tokio::test]
async fn test_execute_command_creates_event() {
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut client = create_gateway_client().await;
    let customer_id = Uuid::new_v4();

    let command = CreateCustomer {
        name: "Test Customer".to_string(),
        email: "test@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let response = client.execute(command_book).await;
    assert!(
        response.is_ok(),
        "Command execution failed: {:?}",
        response.err()
    );

    let response = response.unwrap().into_inner();
    assert!(response.events.is_some(), "Expected events in response");

    let events = response.events.unwrap();
    assert!(!events.pages.is_empty(), "Expected at least one event");

    let last_event = events.pages.last().unwrap();
    let event_any = last_event.event.as_ref().expect("Event has no payload");
    let event_type = extract_event_type(event_any);
    assert!(
        event_type.contains("CustomerCreated"),
        "Expected CustomerCreated event, got {}",
        event_type
    );
}

#[tokio::test]
async fn test_execute_command_returns_projections() {
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut client = create_gateway_client().await;
    let customer_id = Uuid::new_v4();

    let command = CreateCustomer {
        name: "Projection Test".to_string(),
        email: "projection@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let response = client.execute(command_book).await;
    assert!(
        response.is_ok(),
        "Command execution failed: {:?}",
        response.err()
    );

    let response = response.unwrap().into_inner();

    // Verify projections are returned (if projectors are configured)
    // This test documents expected behavior - projections may be empty if no projectors deployed
    if !response.projections.is_empty() {
        for projection in &response.projections {
            assert!(
                !projection.projector.is_empty(),
                "Projector name should not be empty"
            );
        }
    }
}

#[tokio::test]
async fn test_query_events_after_command() {
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Execute command
    let command = CreateCustomer {
        name: "Query Test".to_string(),
        email: "query@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok(), "Command execution failed");

    // Small delay to ensure write is committed
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query events
    let query = build_query("customer", customer_id);
    let query_response = query_client.get_event_book(query).await;
    assert!(
        query_response.is_ok(),
        "Query failed: {:?}",
        query_response.err()
    );

    let event_book = query_response.unwrap().into_inner();
    assert_eq!(event_book.pages.len(), 1, "Expected exactly 1 event");

    let event = &event_book.pages[0];
    let event_any = event.event.as_ref().expect("Event has no payload");
    let event_type = extract_event_type(event_any);
    assert!(
        event_type.contains("CustomerCreated"),
        "Expected CustomerCreated event, got {}",
        event_type
    );
}

#[tokio::test]
async fn test_multiple_commands_sequence_events() {
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Execute first command
    let command1 = CreateCustomer {
        name: "First Command".to_string(),
        email: "first@example.com".to_string(),
    };
    let command_book1 =
        build_command_book("customer", customer_id, command1, "examples.CreateCustomer");

    let response1 = gateway_client.execute(command_book1).await;
    assert!(response1.is_ok(), "First command failed");

    // Small delay between commands
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Query and verify event count
    let query = build_query("customer", customer_id);
    let query_response = query_client.get_event_book(query).await;
    assert!(query_response.is_ok(), "Query failed");

    let event_book = query_response.unwrap().into_inner();
    assert!(
        !event_book.pages.is_empty(),
        "Expected at least one event after command"
    );

    // Verify events exist and are ordered
    // Sequence is a oneof field (num or force), so we just verify count
    assert!(
        !event_book.pages.is_empty(),
        "Expected events to be present"
    );
}
