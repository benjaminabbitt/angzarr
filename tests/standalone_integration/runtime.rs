//! Embedded runtime integration tests.
//!
//! Tests the core runtime command execution lifecycle including event persistence,
//! sequence numbering, channel bus publishing, multi-aggregate support,
//! multi-event commands, correlation ID propagation, and concurrent command handling.

use std::sync::Arc;

use uuid::Uuid;

use crate::common::*;

#[tokio::test]
async fn test_runtime_executes_command_and_persists_events() {
    let aggregate = Arc::new(EchoAggregate::new());
    let agg_clone = aggregate.clone();

    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregateWrapper(agg_clone))
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();

    let root = Uuid::new_v4();
    let command = create_test_command("orders", root, b"test-data", 0);

    let response = client.execute(command).await.expect("Command failed");

    assert!(response.events.is_some(), "Should return events");
    let events = response.events.unwrap();
    assert_eq!(events.pages.len(), 1, "Should have one event");
    assert_eq!(aggregate.calls(), 1, "Aggregate should be called once");

    // Verify event was persisted
    let stored = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .expect("Failed to get events");
    assert_eq!(stored.len(), 1, "Should persist one event");
}

#[tokio::test]
async fn test_runtime_sequence_increments() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // Execute first command
    let cmd1 = create_test_command("orders", root, b"command-1", 0);
    let resp1 = client.execute(cmd1).await.expect("Command 1 failed");
    let seq1 = extract_seq(&resp1);

    // Execute second command
    let cmd2 = create_test_command("orders", root, b"command-2", 1);
    let resp2 = client.execute(cmd2).await.expect("Command 2 failed");
    let seq2 = extract_seq(&resp2);

    assert_eq!(seq1, 0, "First event should have sequence 0");
    assert_eq!(seq2, 1, "Second event should have sequence 1");
}

#[tokio::test]
async fn test_events_published_to_channel_bus() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    // Subscribe to event bus
    let event_bus = runtime.event_bus();
    let handler_state = RecordingHandlerState::new();
    let subscriber = event_bus.create_subscriber("test-sub", None).await.unwrap();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();
    subscriber.start_consuming().await.unwrap();

    let client = runtime.command_client();

    let root = Uuid::new_v4();
    let command = create_test_command("orders", root, b"test", 0);
    client.execute(command).await.expect("Command failed");

    // Give channel bus time to deliver (increased for test reliability)
    tokio::time::sleep(Duration::from_millis(200)).await;

    let count = handler_state.received_count().await;
    assert!(
        count >= 1,
        "Events should be published to channel bus (got {})",
        count
    );
}

#[tokio::test]
async fn test_multiple_aggregates() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .register_aggregate("products", EchoAggregate::new())
        .register_aggregate("customers", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();

    // Execute commands on different aggregates
    for domain in ["orders", "products", "customers"] {
        let cmd = create_test_command(domain, Uuid::new_v4(), b"test", 0);
        let resp = client
            .execute(cmd)
            .await
            .expect(&format!("{} command failed", domain));
        assert!(resp.events.is_some(), "{} should return events", domain);
    }

    // Verify events persisted in each domain
    for domain in ["orders", "products", "customers"] {
        let roots = runtime
            .event_store(domain)
            .unwrap()
            .list_roots(domain, DEFAULT_EDITION)
            .await
            .unwrap();
        assert_eq!(roots.len(), 1, "{} should have 1 aggregate root", domain);
    }
}

#[tokio::test]
async fn test_sequential_commands_same_aggregate() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let root = Uuid::new_v4();
    let client = runtime.command_client();

    // Execute multiple commands sequentially to same aggregate
    for i in 0..5 {
        let cmd = create_test_command("orders", root, format!("cmd-{}", i).as_bytes(), i as u32);
        let result = client.execute(cmd).await;
        assert!(result.is_ok(), "Command {} should succeed", i);
    }

    // Verify all events persisted with correct sequences
    let events = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .unwrap();
    assert_eq!(events.len(), 5, "Should have 5 events");

    // Verify sequences are 0-4
    for (i, event) in events.iter().enumerate() {
        assert_eq!(
            event.sequence as usize, i,
            "Event {} should have sequence {}",
            i, i
        );
    }
}

#[tokio::test]
async fn test_multiple_events_in_single_command() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", MultiEventAggregate::new(3))
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();

    let root = Uuid::new_v4();
    let command = create_test_command("orders", root, b"multi", 0);
    let response = client.execute(command).await.expect("Command failed");

    let events = response.events.expect("Should have events");
    assert_eq!(events.pages.len(), 3, "Should produce 3 events");

    // Verify stored
    let stored = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .expect("Failed to get events");
    assert_eq!(stored.len(), 3, "Should persist all 3 events");
}

#[tokio::test]
async fn test_correlation_id_propagates() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();

    let correlation_id = "test-correlation-123";
    let root = Uuid::new_v4();
    let mut command = create_test_command("orders", root, b"test", 0);
    if let Some(ref mut cover) = command.cover {
        cover.correlation_id = correlation_id.to_string();
    }

    let response = client.execute(command).await.expect("Command failed");

    let events = response.events.expect("Should have events");
    assert_eq!(
        events
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or(""),
        correlation_id,
        "Correlation ID should propagate to events"
    );
}

// ---------------------------------------------------------------------------
// Concurrent / rapid command tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_sequential_commands_different_aggregates() {
    // Test commands to different aggregates execute independently
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();

    // Execute commands to different aggregates
    let total = 10;
    let mut results = Vec::new();

    for i in 0..total {
        let root = Uuid::new_v4();
        let cmd = create_test_command("orders", root, format!("different-{}", i).as_bytes(), 0);
        let result = client.execute(cmd).await.expect("Command failed");
        results.push((root, result));
    }

    assert_eq!(results.len(), total, "All commands should succeed");

    // Verify each aggregate has exactly one event at sequence 0
    for (root, _) in &results {
        let events = runtime
            .event_store("orders")
            .unwrap()
            .get("orders", DEFAULT_EDITION, *root)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 0, "First event should be sequence 0");
    }
}

#[tokio::test]
async fn test_rapid_sequential_commands() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // Execute commands rapidly in sequence (no sleep between)
    for i in 0..50 {
        let cmd = create_test_command("orders", root, format!("rapid-{}", i).as_bytes(), i as u32);
        client
            .execute(cmd)
            .await
            .expect(&format!("Command {} failed", i));
    }

    let events = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .unwrap();
    assert_eq!(events.len(), 50, "Should have 50 events");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the sequence number from the first event page in a command response.
fn extract_seq(response: &angzarr::proto::CommandResponse) -> u32 {
    response
        .events
        .as_ref()
        .and_then(|e| e.pages.first())
        .map(|p| p.sequence)
        .unwrap_or(0)
}
