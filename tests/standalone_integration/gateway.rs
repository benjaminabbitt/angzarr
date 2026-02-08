//! Embedded gateway integration tests.

use crate::common::*;

/// Simple projector for gateway tests.
struct GatewayTestProjector;

#[async_trait]
impl ProjectorHandler for GatewayTestProjector {
    async fn handle(&self, events: &EventBook, _mode: ProjectionMode) -> Result<Projection, Status> {
        Ok(Projection {
            projector: "receipt".to_string(),
            cover: events.cover.clone(),
            projection: Some(Any {
                type_url: "test.Receipt".to_string(),
                value: b"receipt-data".to_vec(),
            }),
            sequence: events.pages.len() as u32,
        })
    }
}

/// Test that command execution returns events like gateway would.
#[tokio::test]
async fn test_execute_returns_events_with_sequence() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    let command = create_test_command("orders", root, b"test-data", 0);
    let response = client.execute(command).await.expect("Command failed");

    // Verify response structure matches gateway expectations
    assert!(response.events.is_some(), "Response should include events");
    let events = response.events.as_ref().unwrap();
    assert!(events.cover.is_some(), "Events should have cover");
    assert_eq!(
        events.cover.as_ref().unwrap().domain,
        "orders",
        "Domain should match"
    );
    assert!(!events.pages.is_empty(), "Should have event pages");

    // Verify sequence is set
    let first_page = &events.pages[0];
    match &first_page.sequence {
        Some(event_page::Sequence::Num(n)) => assert_eq!(*n, 0, "First event should be seq 0"),
        _ => panic!("Expected sequence number"),
    }
}

/// Test query-like access to events after command.
#[tokio::test]
async fn test_query_events_after_command() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // Execute command
    let command = create_test_command("orders", root, b"query-test", 0);
    client.execute(command).await.expect("Command failed");

    // Query events directly from store (like query service would)
    let events = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .expect("Query failed");

    assert_eq!(events.len(), 1, "Should have one event");
    assert!(events[0].event.is_some(), "Event should have payload");
}

/// Test query with bounds (from/to sequence).
#[tokio::test]
async fn test_query_events_with_bounds() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // Execute multiple commands
    for i in 0..5 {
        let command = create_test_command("orders", root, format!("cmd-{}", i).as_bytes(), i as u32);
        client.execute(command).await.expect("Command failed");
    }

    // Query with bounds
    let all_events = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .expect("Query all failed");
    assert_eq!(all_events.len(), 5, "Should have 5 events");

    // Query subset (from seq 2)
    let subset = runtime
        .event_store("orders")
        .unwrap()
        .get_from("orders", DEFAULT_EDITION, root, 2)
        .await
        .expect("Query from failed");
    assert_eq!(subset.len(), 3, "Should have events from seq 2 onwards");

    // Query range
    let range = runtime
        .event_store("orders")
        .unwrap()
        .get_from_to("orders", DEFAULT_EDITION, root, 1, 3)
        .await
        .expect("Query range failed");
    assert_eq!(range.len(), 2, "Should have events 1-2");
}

/// Test multiple commands return proper projections.
#[tokio::test]
async fn test_execute_returns_sync_projections() {
    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .register_projector("receipt", GatewayTestProjector, ProjectorConfig::sync())
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    let command = create_test_command("orders", root, b"projection-test", 0);
    let response = client.execute(command).await.expect("Command failed");

    // Sync projector results should be in response
    assert!(
        !response.projections.is_empty(),
        "Should include sync projector output"
    );
    assert_eq!(
        response.projections[0].projector, "receipt",
        "Projector name should match"
    );
}

/// Test list domains functionality.
#[tokio::test]
async fn test_list_registered_domains() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .register_aggregate("products", EchoAggregate::new())
        .register_aggregate("customers", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let router = runtime.router();
    let domains = router.domains();
    // 3 user domains + 1 auto-registered _angzarr meta domain
    assert_eq!(domains.len(), 4, "Should have 4 domains (3 user + _angzarr)");
    assert!(domains.contains(&"orders"), "Should contain orders");
    assert!(domains.contains(&"products"), "Should contain products");
    assert!(domains.contains(&"customers"), "Should contain customers");
    assert!(domains.contains(&"_angzarr"), "Should contain _angzarr meta domain");
}

/// Test list aggregate roots in domain.
#[tokio::test]
async fn test_list_roots_in_domain() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();

    // Create multiple aggregates
    let roots: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
    for root in &roots {
        let command = create_test_command("orders", *root, b"test", 0);
        client.execute(command).await.expect("Command failed");
    }

    // List roots
    let listed = runtime
        .event_store("orders")
        .unwrap()
        .list_roots("orders", DEFAULT_EDITION)
        .await
        .expect("List failed");

    assert_eq!(listed.len(), 3, "Should list all roots");
    for root in &roots {
        assert!(listed.contains(root), "Should contain root {}", root);
    }
}
