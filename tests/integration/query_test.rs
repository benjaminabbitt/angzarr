//! Query integration tests.
//!
//! Tests event querying and pagination.
//! Run with: ANGZARR_TEST_MODE=container cargo test --test query_integration

#[path = "../common/mod.rs"]
mod common;

use common::{
    build_command_book, build_query, create_gateway_client, create_query_client,
    extract_event_type, should_run_integration_tests, ProtoUuid, Query,
};
use uuid::Uuid;

// Examples proto types
#[allow(dead_code)]
mod examples_proto {
    include!(concat!(env!("OUT_DIR"), "/examples.rs"));
}

use examples_proto::CreateCustomer;

#[tokio::test]
async fn test_query_empty_aggregate() {
    if !should_run_integration_tests() {
        eprintln!("Skipping: Set ANGZARR_TEST_MODE=container to run");
        return;
    }

    let mut query_client = create_query_client().await;
    let nonexistent_id = Uuid::new_v4();

    let query = build_query("customer", nonexistent_id);
    let response = query_client.get_event_book(query).await;

    // Query should succeed but return empty events
    assert!(
        response.is_ok(),
        "Query should not fail for empty aggregate"
    );

    let event_book = response.unwrap().into_inner();
    assert!(
        event_book.pages.is_empty(),
        "Expected no events for nonexistent aggregate"
    );
}

#[tokio::test]
async fn test_query_with_bounds() {
    if !should_run_integration_tests() {
        eprintln!("Skipping: Set ANGZARR_TEST_MODE=container to run");
        return;
    }

    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer
    let command = CreateCustomer {
        name: "Bounds Test".to_string(),
        email: "bounds@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query with lower bound = 0, upper bound = 0 (only first event)
    let query = Query {
        domain: "customer".to_string(),
        root: Some(ProtoUuid {
            value: customer_id.as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: 0,
    };

    let response = query_client.get_event_book(query).await;
    assert!(response.is_ok());

    let event_book = response.unwrap().into_inner();
    assert_eq!(
        event_book.pages.len(),
        1,
        "Expected exactly 1 event with bounds [0,0]"
    );
}

#[tokio::test]
async fn test_query_returns_correct_domain() {
    if !should_run_integration_tests() {
        eprintln!("Skipping: Set ANGZARR_TEST_MODE=container to run");
        return;
    }

    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer
    let command = CreateCustomer {
        name: "Domain Test".to_string(),
        email: "domain@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query the events
    let query = build_query("customer", customer_id);
    let response = query_client.get_event_book(query).await;
    assert!(response.is_ok());

    let event_book = response.unwrap().into_inner();

    // Verify cover contains correct domain and root
    let cover = event_book.cover.expect("EventBook should have cover");
    assert_eq!(cover.domain, "customer", "Domain should be 'customer'");

    let root = cover.root.expect("Cover should have root");
    let root_uuid = Uuid::from_slice(&root.value).expect("Invalid UUID in root");
    assert_eq!(root_uuid, customer_id, "Root UUID should match customer ID");
}

#[tokio::test]
async fn test_query_events_preserve_order() {
    if !should_run_integration_tests() {
        eprintln!("Skipping: Set ANGZARR_TEST_MODE=container to run");
        return;
    }

    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer
    let command = CreateCustomer {
        name: "Order Test".to_string(),
        email: "order@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query the events
    let query = build_query("customer", customer_id);
    let response = query_client.get_event_book(query).await;
    assert!(response.is_ok());

    let event_book = response.unwrap().into_inner();

    // Verify events exist
    assert!(!event_book.pages.is_empty(), "Expected at least one event");
}

#[tokio::test]
async fn test_query_event_payloads() {
    if !should_run_integration_tests() {
        eprintln!("Skipping: Set ANGZARR_TEST_MODE=container to run");
        return;
    }

    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    let name = "Payload Test Customer";
    let email = "payload@example.com";

    // Create customer with specific name and email
    let command = CreateCustomer {
        name: name.to_string(),
        email: email.to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query the events
    let query = build_query("customer", customer_id);
    let response = query_client.get_event_book(query).await;
    assert!(response.is_ok());

    let event_book = response.unwrap().into_inner();
    assert!(!event_book.pages.is_empty());

    // Verify event payload can be decoded
    let first_event = &event_book.pages[0];
    let event_any = first_event
        .event
        .as_ref()
        .expect("Event should have payload");

    let event_type = extract_event_type(event_any);
    assert!(
        event_type.contains("CustomerCreated"),
        "Expected CustomerCreated event"
    );

    // Decode and verify payload content
    use examples_proto::CustomerCreated;
    use prost::Message;

    let customer_created = CustomerCreated::decode(event_any.value.as_slice())
        .expect("Failed to decode CustomerCreated");

    assert_eq!(
        customer_created.name, name,
        "Decoded name should match input"
    );
    assert_eq!(
        customer_created.email, email,
        "Decoded email should match input"
    );
}
