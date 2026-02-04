//! Streaming integration tests.
//!
//! Tests event streaming and subscriptions through the gateway.
//! Run with: ANGZARR_TEST_MODE=container cargo test --test streaming_integration

#[path = "../common/mod.rs"]
mod common;

use common::{build_command_book, create_gateway_client, extract_event_type};
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
async fn test_execute_stream_returns_events() {
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut client = create_gateway_client().await;
    let inventory_id = Uuid::new_v4();

    let command = InitializeStock {
        product_id: "STREAM-SKU".to_string(),
        quantity: 100,
        low_stock_threshold: 10,
    };
    let command_book =
        build_command_book("inventory", inventory_id, command, "examples.InitializeStock");

    let correlation_id = command_book
        .cover
        .as_ref()
        .map(|c| c.correlation_id.clone())
        .unwrap_or_default();

    let response = client.execute_stream(command_book).await;
    assert!(
        response.is_ok(),
        "Stream request failed: {:?}",
        response.err()
    );

    let mut stream = response.unwrap().into_inner();
    let mut events = Vec::new();

    // Collect events with timeout
    let timeout = tokio::time::timeout(tokio::time::Duration::from_secs(5), async {
        while let Ok(Some(event_book)) = stream.message().await {
            events.push(event_book);
            if events.len() >= 5 {
                break;
            }
        }
    });
    let _ = timeout.await;

    assert!(
        !events.is_empty(),
        "Expected at least one event from stream"
    );

    // Verify all events have the same correlation ID
    for event in &events {
        let event_correlation_id = event
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("");
        assert_eq!(
            event_correlation_id, correlation_id,
            "Event correlation ID mismatch"
        );
    }
}

#[tokio::test]
async fn test_stream_includes_expected_event_types() {
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut client = create_gateway_client().await;
    let inventory_id = Uuid::new_v4();

    let command = InitializeStock {
        product_id: "EVENT-TYPE-SKU".to_string(),
        quantity: 50,
        low_stock_threshold: 5,
    };
    let command_book =
        build_command_book("inventory", inventory_id, command, "examples.InitializeStock");

    let response = client.execute_stream(command_book).await;
    assert!(response.is_ok());

    let mut stream = response.unwrap().into_inner();
    let mut found_stock_initialized = false;

    let timeout = tokio::time::timeout(tokio::time::Duration::from_secs(5), async {
        while let Ok(Some(event_book)) = stream.message().await {
            for page in &event_book.pages {
                if let Some(event_any) = &page.event {
                    let event_type = extract_event_type(event_any);
                    if event_type.contains("StockInitialized") {
                        found_stock_initialized = true;
                        return;
                    }
                }
            }
        }
    });
    let _ = timeout.await;

    assert!(
        found_stock_initialized,
        "Expected to receive StockInitialized event in stream"
    );
}

#[tokio::test]
async fn test_execute_returns_immediate_response() {
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut client = create_gateway_client().await;
    let inventory_id = Uuid::new_v4();

    let command = InitializeStock {
        product_id: "UNARY-SKU".to_string(),
        quantity: 75,
        low_stock_threshold: 8,
    };
    let command_book =
        build_command_book("inventory", inventory_id, command, "examples.InitializeStock");

    let response = client.execute(command_book).await;
    assert!(response.is_ok());

    let cmd_response = response.unwrap().into_inner();
    assert!(cmd_response.events.is_some(), "Expected events in response");
}

#[tokio::test]
async fn test_multiple_inventories_isolated_streams() {
    if !should_run_container_tests() {
        println!("Skipping: set ANGZARR_TEST_MODE=container to run");
        return;
    }
    let mut client = create_gateway_client().await;
    let inventory_id_1 = Uuid::new_v4();
    let inventory_id_2 = Uuid::new_v4();

    // Initialize first inventory
    let command1 = InitializeStock {
        product_id: "ISO-SKU-1".to_string(),
        quantity: 100,
        low_stock_threshold: 10,
    };
    let command_book1 = build_command_book(
        "inventory",
        inventory_id_1,
        command1,
        "examples.InitializeStock",
    );
    let correlation_id_1 = command_book1
        .cover
        .as_ref()
        .map(|c| c.correlation_id.clone())
        .unwrap_or_default();

    let response1 = client.execute_stream(command_book1).await;
    assert!(response1.is_ok());

    let mut stream1 = response1.unwrap().into_inner();
    let mut events1 = Vec::new();
    let timeout1 = tokio::time::timeout(tokio::time::Duration::from_secs(2), async {
        while let Ok(Some(event_book)) = stream1.message().await {
            events1.push(event_book);
            if events1.len() >= 5 {
                break;
            }
        }
    });
    let _ = timeout1.await;

    // Initialize second inventory
    let command2 = InitializeStock {
        product_id: "ISO-SKU-2".to_string(),
        quantity: 200,
        low_stock_threshold: 20,
    };
    let command_book2 = build_command_book(
        "inventory",
        inventory_id_2,
        command2,
        "examples.InitializeStock",
    );
    let correlation_id_2 = command_book2
        .cover
        .as_ref()
        .map(|c| c.correlation_id.clone())
        .unwrap_or_default();

    let response2 = client.execute_stream(command_book2).await;
    assert!(response2.is_ok());

    let mut stream2 = response2.unwrap().into_inner();
    let mut events2 = Vec::new();
    let timeout2 = tokio::time::timeout(tokio::time::Duration::from_secs(2), async {
        while let Ok(Some(event_book)) = stream2.message().await {
            events2.push(event_book);
            if events2.len() >= 5 {
                break;
            }
        }
    });
    let _ = timeout2.await;

    // Verify events are isolated by correlation ID
    for event in &events1 {
        let event_correlation_id = event
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("");
        assert_eq!(
            event_correlation_id, correlation_id_1,
            "Inventory 1 stream contains wrong correlation ID"
        );
    }

    for event in &events2 {
        let event_correlation_id = event
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("");
        assert_eq!(
            event_correlation_id, correlation_id_2,
            "Inventory 2 stream contains wrong correlation ID"
        );
    }
}
