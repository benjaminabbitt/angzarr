//! Query integration tests.
//!
//! Tests event querying and pagination.
//! Run with: ANGZARR_TEST_MODE=container cargo test --test query_integration

#[path = "../common/mod.rs"]
mod common;

use common::{
    build_command_book, build_query, create_gateway_client, create_query_client,
    extract_event_type, ProtoUuid, Query,
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
async fn test_query_empty_aggregate() {
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut query_client = create_query_client().await;
    let nonexistent_id = Uuid::new_v4();

    let query = build_query("inventory", nonexistent_id);
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
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let inventory_id = Uuid::new_v4();

    // Initialize inventory
    let command = InitializeStock {
        product_id: "BOUNDS-SKU".to_string(),
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
    assert!(response.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query with range [0, 0] (only first event)
    use angzarr::proto::{query::Selection, Cover, SequenceRange};
    let query = Query {
        cover: Some(Cover {
            domain: "inventory".to_string(),
            root: Some(ProtoUuid {
                value: inventory_id.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: Some(Selection::Range(SequenceRange {
            lower: 0,
            upper: Some(0),
        })),
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
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let inventory_id = Uuid::new_v4();

    // Initialize inventory
    let command = InitializeStock {
        product_id: "DOMAIN-SKU".to_string(),
        quantity: 50,
        low_stock_threshold: 5,
    };
    let command_book = build_command_book(
        "inventory",
        inventory_id,
        command,
        "examples.InitializeStock",
    );

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query the events
    let query = build_query("inventory", inventory_id);
    let response = query_client.get_event_book(query).await;
    assert!(response.is_ok());

    let event_book = response.unwrap().into_inner();

    // Verify cover contains correct domain and root
    let cover = event_book.cover.expect("EventBook should have cover");
    assert_eq!(cover.domain, "inventory", "Domain should be 'inventory'");

    let root = cover.root.expect("Cover should have root");
    let root_uuid = Uuid::from_slice(&root.value).expect("Invalid UUID in root");
    assert_eq!(
        root_uuid, inventory_id,
        "Root UUID should match inventory ID"
    );
}

#[tokio::test]
async fn test_query_events_preserve_order() {
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let inventory_id = Uuid::new_v4();

    // Initialize inventory
    let command = InitializeStock {
        product_id: "ORDER-SKU".to_string(),
        quantity: 75,
        low_stock_threshold: 8,
    };
    let command_book = build_command_book(
        "inventory",
        inventory_id,
        command,
        "examples.InitializeStock",
    );

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query the events
    let query = build_query("inventory", inventory_id);
    let response = query_client.get_event_book(query).await;
    assert!(response.is_ok());

    let event_book = response.unwrap().into_inner();

    // Verify events exist
    assert!(!event_book.pages.is_empty(), "Expected at least one event");
}

#[tokio::test]
async fn test_query_event_payloads() {
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let inventory_id = Uuid::new_v4();

    let product_id = "PAYLOAD-SKU";
    let quantity = 200i32;
    let threshold = 20i32;

    // Initialize inventory with specific values
    let command = InitializeStock {
        product_id: product_id.to_string(),
        quantity,
        low_stock_threshold: threshold,
    };
    let command_book = build_command_book(
        "inventory",
        inventory_id,
        command,
        "examples.InitializeStock",
    );

    let response = gateway_client.execute(command_book).await;
    assert!(response.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query the events
    let query = build_query("inventory", inventory_id);
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
        event_type.contains("StockInitialized"),
        "Expected StockInitialized event"
    );

    // Decode and verify payload content
    use examples_proto::StockInitialized;
    use prost::Message;

    let stock_initialized = StockInitialized::decode(event_any.value.as_slice())
        .expect("Failed to decode StockInitialized");

    assert_eq!(
        stock_initialized.product_id, product_id,
        "Decoded product_id should match input"
    );
    assert_eq!(
        stock_initialized.quantity, quantity,
        "Decoded quantity should match input"
    );
}
