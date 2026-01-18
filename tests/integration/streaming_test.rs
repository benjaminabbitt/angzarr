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

use examples_proto::CreateCustomer;

#[tokio::test]
async fn test_execute_stream_returns_events() {
    let mut client = create_gateway_client().await;
    let customer_id = Uuid::new_v4();

    let command = CreateCustomer {
        name: "Stream Test".to_string(),
        email: "stream@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let correlation_id = command_book.correlation_id.clone();

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
        assert_eq!(
            event.correlation_id, correlation_id,
            "Event correlation ID mismatch"
        );
    }
}

#[tokio::test]
async fn test_stream_includes_expected_event_types() {
    let mut client = create_gateway_client().await;
    let customer_id = Uuid::new_v4();

    let command = CreateCustomer {
        name: "Event Type Test".to_string(),
        email: "eventtype@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let response = client.execute_stream(command_book).await;
    assert!(response.is_ok());

    let mut stream = response.unwrap().into_inner();
    let mut found_customer_created = false;

    let timeout = tokio::time::timeout(tokio::time::Duration::from_secs(5), async {
        while let Ok(Some(event_book)) = stream.message().await {
            for page in &event_book.pages {
                if let Some(event_any) = &page.event {
                    let event_type = extract_event_type(event_any);
                    if event_type.contains("CustomerCreated") {
                        found_customer_created = true;
                        return;
                    }
                }
            }
        }
    });
    let _ = timeout.await;

    assert!(
        found_customer_created,
        "Expected to receive CustomerCreated event in stream"
    );
}

#[tokio::test]
async fn test_execute_returns_immediate_response() {
    let mut client = create_gateway_client().await;
    let customer_id = Uuid::new_v4();

    let command = CreateCustomer {
        name: "Unary Test".to_string(),
        email: "unary@example.com".to_string(),
    };
    let command_book =
        build_command_book("customer", customer_id, command, "examples.CreateCustomer");

    let response = client.execute(command_book).await;
    assert!(response.is_ok());

    let cmd_response = response.unwrap().into_inner();
    assert!(cmd_response.events.is_some(), "Expected events in response");
}

#[tokio::test]
async fn test_multiple_customers_isolated_streams() {
    let mut client = create_gateway_client().await;
    let customer_id_1 = Uuid::new_v4();
    let customer_id_2 = Uuid::new_v4();

    // Create first customer
    let command1 = CreateCustomer {
        name: "Customer One".to_string(),
        email: "one@example.com".to_string(),
    };
    let command_book1 = build_command_book(
        "customer",
        customer_id_1,
        command1,
        "examples.CreateCustomer",
    );
    let correlation_id_1 = command_book1.correlation_id.clone();

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

    // Create second customer
    let command2 = CreateCustomer {
        name: "Customer Two".to_string(),
        email: "two@example.com".to_string(),
    };
    let command_book2 = build_command_book(
        "customer",
        customer_id_2,
        command2,
        "examples.CreateCustomer",
    );
    let correlation_id_2 = command_book2.correlation_id.clone();

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
        assert_eq!(
            event.correlation_id, correlation_id_1,
            "Customer 1 stream contains wrong correlation ID"
        );
    }

    for event in &events2 {
        assert_eq!(
            event.correlation_id, correlation_id_2,
            "Customer 2 stream contains wrong correlation ID"
        );
    }
}
