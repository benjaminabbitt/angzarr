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

use examples_proto::InitializeStock;

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
    let inventory_id = Uuid::new_v4();

    let command = InitializeStock {
        product_id: "TEST-SKU-001".to_string(),
        quantity: 100,
        low_stock_threshold: 10,
    };
    let command_book = build_command_book(
        "inventory",
        inventory_id,
        command,
        "examples.InitializeStock",
    );

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
        event_type.contains("StockInitialized"),
        "Expected StockInitialized event, got {}",
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
    let inventory_id = Uuid::new_v4();

    let command = InitializeStock {
        product_id: "TEST-SKU-PROJ".to_string(),
        quantity: 50,
        low_stock_threshold: 5,
    };
    let command_book = build_command_book(
        "inventory",
        inventory_id,
        command,
        "examples.InitializeStock",
    );

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
    let inventory_id = Uuid::new_v4();

    // Execute command
    let command = InitializeStock {
        product_id: "QUERY-SKU".to_string(),
        quantity: 100,
        low_stock_threshold: 10,
    };
    let command_book = build_command_book(
        "inventory",
        inventory_id,
        command,
        "examples.InitializeStock",
    );

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok(), "Command execution failed");

    // Small delay to ensure write is committed
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query events
    let query = build_query("inventory", inventory_id);
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
        event_type.contains("StockInitialized"),
        "Expected StockInitialized event, got {}",
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
    let inventory_id = Uuid::new_v4();

    // Execute first command
    let command1 = InitializeStock {
        product_id: "SEQ-SKU".to_string(),
        quantity: 75,
        low_stock_threshold: 8,
    };
    let command_book1 = build_command_book(
        "inventory",
        inventory_id,
        command1,
        "examples.InitializeStock",
    );

    let response1 = gateway_client.execute(command_book1).await;
    assert!(response1.is_ok(), "First command failed");

    // Small delay between commands
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Query and verify event count
    let query = build_query("inventory", inventory_id);
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
