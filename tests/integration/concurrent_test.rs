//! Concurrent command integration tests.
//!
//! Tests concurrent command handling, race conditions, and conflict resolution.

#[path = "../common/mod.rs"]
mod common;

use common::{
    build_command_book, build_command_book_auto_resequence, build_command_book_with_options,
    build_query, create_gateway_client, create_query_client, extract_sequence,
};
use std::sync::Arc;
use tokio::sync::Barrier;
use uuid::Uuid;

// Examples proto types
#[allow(dead_code)]
mod examples_proto {
    include!(concat!(env!("OUT_DIR"), "/examples.rs"));
}

use examples_proto::{AddLoyaltyPoints, CreateCustomer};

/// Tests that concurrent commands with auto_resequence all succeed.
#[tokio::test]
async fn test_concurrent_commands_with_auto_resequence() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer first
    let create = CreateCustomer {
        name: "Concurrent Test".to_string(),
        email: "concurrent@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Launch multiple concurrent commands with auto_resequence
    let num_concurrent = 5;
    let barrier = Arc::new(Barrier::new(num_concurrent));
    let mut handles = Vec::new();

    for i in 0..num_concurrent {
        let customer_id = customer_id;
        let barrier = Arc::clone(&barrier);

        let handle = tokio::spawn(async move {
            let mut client = create_gateway_client().await;
            let add_points = AddLoyaltyPoints {
                points: (i + 1) as i32 * 100,
                reason: format!("concurrent-{}", i),
            };
            let cmd = build_command_book_auto_resequence(
                "customer",
                customer_id,
                add_points,
                "examples.AddLoyaltyPoints",
            );

            // Wait for all tasks to be ready
            barrier.wait().await;

            // Execute concurrently
            client.execute(cmd).await
        });

        handles.push(handle);
    }

    // Collect results
    let mut success_count = 0;
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => success_count += 1,
            Ok(Err(e)) => eprintln!("Command failed: {:?}", e),
            Err(e) => eprintln!("Task panicked: {:?}", e),
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // With auto_resequence, all commands should succeed
    assert_eq!(
        success_count, num_concurrent,
        "All {} concurrent commands should succeed with auto_resequence",
        num_concurrent
    );

    // Verify event count
    let query = build_query("customer", customer_id);
    let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

    assert_eq!(
        event_book.pages.len(),
        1 + num_concurrent,
        "Should have {} events (1 create + {} adds)",
        1 + num_concurrent,
        num_concurrent
    );

    // Verify sequences are contiguous
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

/// Tests that concurrent commands WITHOUT auto_resequence have conflicts.
#[tokio::test]
async fn test_concurrent_commands_without_auto_resequence_have_conflicts() {
    let mut gateway_client = create_gateway_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer first
    let create = CreateCustomer {
        name: "Conflict Test".to_string(),
        email: "conflict@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Launch multiple concurrent commands all claiming sequence 1
    let num_concurrent = 5;
    let barrier = Arc::new(Barrier::new(num_concurrent));
    let mut handles = Vec::new();

    for i in 0..num_concurrent {
        let customer_id = customer_id;
        let barrier = Arc::clone(&barrier);

        let handle = tokio::spawn(async move {
            let mut client = create_gateway_client().await;
            let add_points = AddLoyaltyPoints {
                points: (i + 1) as i32 * 100,
                reason: format!("conflict-{}", i),
            };
            // All commands claim sequence 1 - only one should succeed
            let cmd = build_command_book_with_options(
                "customer",
                customer_id,
                add_points,
                "examples.AddLoyaltyPoints",
                1,     // Same sequence for all
                false, // No auto_resequence
            );

            // Wait for all tasks to be ready
            barrier.wait().await;

            // Execute concurrently
            client.execute(cmd).await
        });

        handles.push(handle);
    }

    // Collect results
    let mut success_count = 0;
    let mut failure_count = 0;
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => success_count += 1,
            Ok(Err(_)) => failure_count += 1,
            Err(e) => eprintln!("Task panicked: {:?}", e),
        }
    }

    // Only one should succeed, others should fail with sequence conflict
    // (or all could fail if there's additional concurrency control)
    assert!(
        success_count <= 1,
        "At most 1 command should succeed: got {}",
        success_count
    );
    assert!(
        failure_count >= num_concurrent - 1,
        "At least {} commands should fail: got {}",
        num_concurrent - 1,
        failure_count
    );
}

/// Tests high-concurrency stress test with auto_resequence.
#[tokio::test]
async fn test_high_concurrency_stress() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer first
    let create = CreateCustomer {
        name: "Stress Test".to_string(),
        email: "stress@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Launch many concurrent commands
    let num_concurrent = 20;
    let barrier = Arc::new(Barrier::new(num_concurrent));
    let mut handles = Vec::new();

    for i in 0..num_concurrent {
        let customer_id = customer_id;
        let barrier = Arc::clone(&barrier);

        let handle = tokio::spawn(async move {
            let mut client = create_gateway_client().await;
            let add_points = AddLoyaltyPoints {
                points: i as i32 + 1,
                reason: format!("stress-{}", i),
            };
            let cmd = build_command_book_auto_resequence(
                "customer",
                customer_id,
                add_points,
                "examples.AddLoyaltyPoints",
            );

            // Wait for all tasks to be ready
            barrier.wait().await;

            // Execute concurrently
            client.execute(cmd).await
        });

        handles.push(handle);
    }

    // Collect results
    let mut success_count = 0;
    for handle in handles {
        if let Ok(Ok(_)) = handle.await {
            success_count += 1;
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Most commands should succeed with auto_resequence
    // Some may fail due to max retry exhaustion under extreme contention
    assert!(
        success_count >= num_concurrent / 2,
        "At least half of {} commands should succeed: got {}",
        num_concurrent,
        success_count
    );

    // Verify sequences are valid (no gaps, no duplicates)
    let query = build_query("customer", customer_id);
    let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

    let mut sequences: Vec<u32> = event_book
        .pages
        .iter()
        .map(|p| extract_sequence(p))
        .collect();
    sequences.sort();

    // Verify no duplicates
    let mut prev = None;
    for seq in &sequences {
        if let Some(p) = prev {
            assert!(
                *seq > p,
                "Duplicate or out-of-order sequence detected: {} followed by {}",
                p,
                seq
            );
        }
        prev = Some(*seq);
    }

    // Verify sequences are contiguous from 0
    for (i, seq) in sequences.iter().enumerate() {
        assert_eq!(
            *seq, i as u32,
            "Sequence gap detected: expected {}, got {}",
            i, seq
        );
    }
}

/// Tests that different aggregates can be modified concurrently without conflicts.
#[tokio::test]
async fn test_different_aggregates_concurrent() {
    let mut query_client = create_query_client().await;

    // Create multiple aggregates concurrently
    let num_aggregates = 10;
    let barrier = Arc::new(Barrier::new(num_aggregates));
    let mut handles = Vec::new();
    let customer_ids: Vec<Uuid> = (0..num_aggregates).map(|_| Uuid::new_v4()).collect();

    for (i, customer_id) in customer_ids.iter().enumerate() {
        let customer_id = *customer_id;
        let barrier = Arc::clone(&barrier);

        let handle = tokio::spawn(async move {
            let mut client = create_gateway_client().await;
            let create = CreateCustomer {
                name: format!("Customer {}", i),
                email: format!("customer{}@example.com", i),
            };
            let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");

            // Wait for all tasks to be ready
            barrier.wait().await;

            // Execute concurrently
            client.execute(cmd).await.map(|_| customer_id)
        });

        handles.push(handle);
    }

    // All should succeed - different aggregates don't conflict
    let mut created_ids = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(id)) => created_ids.push(id),
            Ok(Err(e)) => panic!("Different aggregate creation should not fail: {:?}", e),
            Err(e) => panic!("Task panicked: {:?}", e),
        }
    }

    assert_eq!(
        created_ids.len(),
        num_aggregates,
        "All aggregates should be created"
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify each aggregate has exactly one event
    for customer_id in &created_ids {
        let query = build_query("customer", *customer_id);
        let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

        assert_eq!(
            event_book.pages.len(),
            1,
            "Each aggregate should have exactly 1 event"
        );
        assert_eq!(
            extract_sequence(&event_book.pages[0]),
            0,
            "First event should be at sequence 0"
        );
    }
}

/// Tests rapid sequential commands to same aggregate.
#[tokio::test]
async fn test_rapid_sequential_commands() {
    let mut gateway_client = create_gateway_client().await;
    let mut query_client = create_query_client().await;
    let customer_id = Uuid::new_v4();

    // Create customer
    let create = CreateCustomer {
        name: "Rapid Sequential".to_string(),
        email: "rapid@example.com".to_string(),
    };
    let cmd = build_command_book("customer", customer_id, create, "examples.CreateCustomer");
    gateway_client.execute(cmd).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Send many commands as fast as possible with auto_resequence
    let num_commands = 10;
    for i in 0..num_commands {
        let add = AddLoyaltyPoints {
            points: i + 1,
            reason: format!("rapid-{}", i),
        };
        let cmd = build_command_book_auto_resequence(
            "customer",
            customer_id,
            add,
            "examples.AddLoyaltyPoints",
        );

        let response = gateway_client.execute(cmd).await;
        assert!(
            response.is_ok(),
            "Rapid command {} should succeed: {:?}",
            i,
            response.err()
        );
        // No sleep between commands
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify all events created with correct sequences
    let query = build_query("customer", customer_id);
    let event_book = query_client.get_event_book(query).await.unwrap().into_inner();

    assert_eq!(
        event_book.pages.len(),
        1 + num_commands as usize,
        "Should have all events"
    );

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
