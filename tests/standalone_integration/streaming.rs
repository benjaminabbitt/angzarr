//! Event streaming integration tests.

use crate::common::*;
use angzarr::proto::Projection;
use angzarr::standalone::{ProjectionMode, ProjectorConfig, ProjectorHandler};

/// Projector that produces streaming output.
struct StreamingProjector;

#[async_trait]
impl ProjectorHandler for StreamingProjector {
    async fn handle(&self, events: &EventBook, _mode: ProjectionMode) -> Result<Projection, Status> {
        Ok(Projection {
            projector: "streaming".to_string(),
            cover: events.cover.clone(),
            projection: Some(Any {
                type_url: "test.StreamedProjection".to_string(),
                value: format!("streamed-{}", events.pages.len()).into_bytes(),
            }),
            sequence: events.pages.len() as u32,
        })
    }
}

#[tokio::test]
async fn test_events_published_to_bus_for_streaming() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    // Subscribe to events
    let event_bus = runtime.event_bus();
    let handler_state = RecordingHandlerState::new();
    let subscriber = event_bus.create_subscriber("test-sub", None).await.unwrap();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();
    subscriber.start_consuming().await.unwrap();

    // Wait for consumer task to be ready
    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = runtime.command_client();

    // Execute multiple commands
    for i in 0..3 {
        let cmd =
            create_test_command("orders", Uuid::new_v4(), format!("stream-{}", i).as_bytes(), 0);
        client.execute(cmd).await.expect("Command failed");
    }

    // Wait for async event distribution to complete
    tokio::time::sleep(Duration::from_millis(200)).await;

    let events = handler_state.get_events().await;
    assert_eq!(
        events.len(),
        3,
        "Should receive all 3 events for streaming (got {})",
        events.len()
    );
}

#[tokio::test]
async fn test_streaming_preserves_correlation_id() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .register_projector("streaming", StreamingProjector, ProjectorConfig::sync())
        .build()
        .await
        .expect("Failed to build runtime");

    let event_bus = runtime.event_bus();
    let handler_state = RecordingHandlerState::new();
    let subscriber = event_bus.create_subscriber("test-sub", None).await.unwrap();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();
    subscriber.start_consuming().await.unwrap();

    let client = runtime.command_client();

    let correlation_id = "streaming-correlation-123";
    let mut command = create_test_command("orders", Uuid::new_v4(), b"stream-test", 0);
    if let Some(ref mut cover) = command.cover {
        cover.correlation_id = correlation_id.to_string();
    }

    let response = client.execute(command).await.expect("Command failed");

    // Response events should have correlation ID
    let response_correlation_id = response
        .events
        .as_ref()
        .and_then(|e| e.cover.as_ref())
        .map(|c| c.correlation_id.as_str())
        .unwrap_or("");
    assert_eq!(response_correlation_id, correlation_id);

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Events on bus should have correlation ID
    let events = handler_state.get_events().await;
    for event in &events {
        let event_correlation_id = event
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("");
        assert_eq!(
            event_correlation_id, correlation_id,
            "Streamed events should preserve correlation ID"
        );
    }
}

#[tokio::test]
async fn test_multiple_subscribers_receive_streamed_events() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let event_bus = runtime.event_bus();

    // Create two subscribers
    let state_a = RecordingHandlerState::new();
    let subscriber_a = event_bus.create_subscriber("test-sub-a", None).await.unwrap();
    subscriber_a
        .subscribe(Box::new(RecordingHandler::new(state_a.clone())))
        .await
        .unwrap();
    subscriber_a.start_consuming().await.unwrap();

    let state_b = RecordingHandlerState::new();
    let subscriber_b = event_bus.create_subscriber("test-sub-b", None).await.unwrap();
    subscriber_b
        .subscribe(Box::new(RecordingHandler::new(state_b.clone())))
        .await
        .unwrap();
    subscriber_b.start_consuming().await.unwrap();

    let client = runtime.command_client();

    let command = create_test_command("orders", Uuid::new_v4(), b"multi-stream", 0);
    client.execute(command).await.expect("Command failed");

    tokio::time::sleep(Duration::from_millis(100)).await;

    let count_a = state_a.received_count().await;
    let count_b = state_b.received_count().await;

    assert!(count_a >= 1, "Subscriber A should receive streamed event");
    assert!(count_b >= 1, "Subscriber B should receive streamed event");
}

#[tokio::test]
async fn test_streamed_events_include_all_pages() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", MultiEventAggregate::new(5))
        .build()
        .await
        .expect("Failed to build runtime");

    let event_bus = runtime.event_bus();
    let handler_state = RecordingHandlerState::new();
    let subscriber = event_bus.create_subscriber("test-sub", None).await.unwrap();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();
    subscriber.start_consuming().await.unwrap();

    let client = runtime.command_client();

    let command = create_test_command("orders", Uuid::new_v4(), b"multi-page", 0);
    client.execute(command).await.expect("Command failed");

    tokio::time::sleep(Duration::from_millis(100)).await;

    let events = handler_state.get_events().await;
    assert!(!events.is_empty(), "Should receive events");

    // The streamed EventBook should contain all 5 pages
    let event_book = &events[0];
    assert_eq!(
        event_book.pages.len(),
        5,
        "Streamed EventBook should include all event pages"
    );
}
