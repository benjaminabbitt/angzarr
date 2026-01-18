//! Auto-resequence integration tests.
//!
//! Tests auto_resequence flag behavior for handling sequence conflicts.

#[path = "../common/mod.rs"]
mod common;

use common::{
    build_command_book, build_command_book_at_sequence, build_command_book_auto_resequence,
    build_query, create_gateway_client, create_query_client, extract_sequence,
};
use uuid::Uuid;

// Examples proto types
#[allow(dead_code)]
mod examples_proto {
    include!(concat!(env!("OUT_DIR"), "/examples.rs"));
}

use examples_proto::{AddLoyaltyPoints, CreateCustomer};

/// Tests that auto_resequence allows command to succeed even with stale sequence.
#[tokio::test]
async fn test_auto_resequence_succeeds_with_stale_sequence() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer first
    let create = CreateCustomer {
        name: "Auto Resequence Test".to_string(),
        email: "autoreseq@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Send command with auto_resequence=true and seq=0 (stale)
    // The system should automatically resequence to seq=1
    let add_points = AddLoyaltyPoints {
        points: 100,
        reason: "auto_reseq".to_string(),
    };
    let cmd =
        build_command_book_auto_resequence("customer", customer_id, add_points, "examples.AddLoyaltyPoints");

    let response = gateway_client.execute(cmd).await;
    assert!(
        response.is_ok(),
        "Auto-resequence command should succeed: {:?}",
        response.err()
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify event was created with correct sequence
    let query = build_query("customer", customer_id);
    let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

    assert_eq!(event_book.pages.len(), 2, "Should have 2 events");
    assert_eq!(extract_sequence(&event_book.pages[0]), 0, "First event at seq 0");
    assert_eq!(extract_sequence(&event_book.pages[1]), 1, "Second event at seq 1");
}

/// Tests that multiple auto_resequence commands work correctly.
#[tokio::test]
async fn test_multiple_auto_resequence_commands() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer
    let create = CreateCustomer {
        name: "Multi Auto Reseq".to_string(),
        email: "multiauto@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Send 5 commands all with auto_resequence=true and seq=0
    for i in 1..=5 {
        let add_points = AddLoyaltyPoints {
            points: i * 10,
            reason: format!("multi-{}", i),
        };
        let cmd = build_command_book_auto_resequence(
            "customer",
            customer_id,
            add_points,
            "examples.AddLoyaltyPoints",
        );

        let response = gateway_client.execute(cmd).await;
        assert!(
            response.is_ok(),
            "Auto-resequence command {} should succeed: {:?}",
            i,
            response.err()
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify all events were created with correct sequences
    let query = build_query("customer", customer_id);
    let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

    assert_eq!(event_book.pages.len(), 6, "Should have 6 events (1 create + 5 adds)");

    for (i, page) in event_book.pages.iter().enumerate() {
        assert_eq!(
            extract_sequence(page),
            i as u32,
            "Event {} should have sequence {}",
            i,
            i
        );
    }
}

/// Tests that auto_resequence works for the first command on a new aggregate.
#[tokio::test]
async fn test_auto_resequence_on_new_aggregate() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer with auto_resequence (new aggregate)
    let create = CreateCustomer {
        name: "New Aggregate Reseq".to_string(),
        email: "newaggregate@example.com".to_string(),
    };
    let cmd =
        build_command_book_auto_resequence("customer", customer_id, create, "examples.CreateCustomer");

    let response = gateway_client.execute(cmd).await;
    assert!(
        response.is_ok(),
        "Auto-resequence on new aggregate should succeed: {:?}",
        response.err()
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify event was created at sequence 0
    let query = build_query("customer", customer_id);
    let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

    assert_eq!(event_book.pages.len(), 1, "Should have 1 event");
    assert_eq!(extract_sequence(&event_book.pages[0]), 0, "First event at seq 0");
}

/// Tests that without auto_resequence, stale sequence is rejected.
#[tokio::test]
async fn test_without_auto_resequence_stale_sequence_rejected() {
    let mut gateway_client = create_gateway_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer
    let create = CreateCustomer {
        name: "No Reseq Test".to_string(),
        email: "noreseq@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Send command WITHOUT auto_resequence and with seq=0 (stale)
    let add_points = AddLoyaltyPoints {
        points: 100,
        reason: "no_reseq".to_string(),
    };
    // Build manually with auto_resequence=false
    let cmd = build_command_book_at_sequence(
        "customer",
        customer_id,
        add_points,
        "examples.AddLoyaltyPoints",
        0, // Wrong sequence - should be 1
    );

    let response = gateway_client.execute(cmd).await;
    assert!(
        response.is_err(),
        "Command with stale sequence should fail without auto_resequence"
    );
}

/// Tests that auto_resequence preserves command data after resequencing.
#[tokio::test]
async fn test_auto_resequence_preserves_command_data() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer
    let create = CreateCustomer {
        name: "Data Preserve Test".to_string(),
        email: "preserve@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Add points with specific values
    let expected_points = 12345;
    let expected_reason = "special_reason_xyz".to_string();
    let add_points = AddLoyaltyPoints {
        points: expected_points,
        reason: expected_reason.clone(),
    };
    let cmd = build_command_book_auto_resequence(
        "customer",
        customer_id,
        add_points,
        "examples.AddLoyaltyPoints",
    );

    let response = gateway_client.execute(cmd).await;
    assert!(response.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Query and verify the event data was preserved
    let query = build_query("customer", customer_id);
    let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

    assert_eq!(event_book.pages.len(), 2);

    // Decode the second event and verify its data
    let event_any = event_book.pages[1].event.as_ref().unwrap();
    let loyalty_event =
        prost::Message::decode::<examples_proto::LoyaltyPointsAdded>(event_any.value.as_slice())
            .expect("Failed to decode LoyaltyPointsAdded");

    assert_eq!(
        loyalty_event.points, expected_points,
        "Points should be preserved after resequence"
    );
    assert_eq!(
        loyalty_event.reason, expected_reason,
        "Reason should be preserved after resequence"
    );
}

/// Tests that correlation ID is preserved through auto_resequence.
#[tokio::test]
async fn test_auto_resequence_preserves_correlation_id() {
    let mut gateway_client = create_gateway_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer first
    let create = CreateCustomer {
        name: "Correlation Test".to_string(),
        email: "correlation@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Add points with auto_resequence
    let add_points = AddLoyaltyPoints {
        points: 100,
        reason: "correlation".to_string(),
    };
    let cmd = build_command_book_auto_resequence(
        "customer",
        customer_id,
        add_points,
        "examples.AddLoyaltyPoints",
    );
    let expected_correlation_id = cmd.correlation_id.clone();

    let response = gateway_client.execute(cmd).await;
    assert!(response.is_ok());

    let response = response.unwrap().into_inner();

    // Verify events have the expected correlation ID
    if let Some(events) = response.events {
        assert_eq!(
            events.correlation_id, expected_correlation_id,
            "Correlation ID should be preserved"
        );
    }
}

/// Tests that auto_resequence works after multiple rapid commands.
#[tokio::test]
async fn test_auto_resequence_after_rapid_commands() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer
    let create = CreateCustomer {
        name: "Rapid Commands".to_string(),
        email: "rapid@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();

    // Add several points with correct sequences
    for i in 1..=3 {
        let add = AddLoyaltyPoints {
            points: i * 10,
            reason: format!("rapid-{}", i),
        };
        let cmd = build_command_book_at_sequence(
            "customer",
            customer_id,
            add,
            "examples.AddLoyaltyPoints",
            i as u32,
        );
        gateway_client.execute(cmd).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Now send an auto_resequence command with stale sequence (0)
    // It should automatically use sequence 4
    let add_stale = AddLoyaltyPoints {
        points: 999,
        reason: "stale_but_resequenced".to_string(),
    };
    let cmd = build_command_book_auto_resequence(
        "customer",
        customer_id,
        add_stale,
        "examples.AddLoyaltyPoints",
    );

    let response = gateway_client.execute(cmd).await;
    assert!(response.is_ok(), "Auto-resequence should handle stale sequence");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify final event count and sequence
    let query = build_query("customer", customer_id);
    let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

    assert_eq!(event_book.pages.len(), 5, "Should have 5 events total");
    assert_eq!(
        extract_sequence(&event_book.pages[4]),
        4,
        "Last event should be at sequence 4"
    );
}
