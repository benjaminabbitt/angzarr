//! MergeStrategy (concurrency control) integration tests.
//!
//! Tests the aggregate coordinator's handling of sequence conflicts based on
//! the MergeStrategy field in CommandPage.
//!
//! See also: examples/features/unit/merge_strategy.feature for Gherkin specs.

use crate::common::*;
use angzarr::proto::MergeStrategy;
use std::sync::atomic::AtomicBool;

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a command with specific merge strategy.
fn create_command_with_strategy(
    domain: &str,
    root: Uuid,
    sequence: u32,
    strategy: MergeStrategy,
) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: Uuid::new_v4().to_string(),
            edition: None,
        }),
        pages: vec![CommandPage {
            sequence,
            command: Some(Any {
                type_url: "test.TestCommand".to_string(),
                value: vec![1, 2, 3],
            }),
            merge_strategy: strategy as i32,
        }],
        saga_origin: None,
    }
}

/// Aggregate that tracks whether it was invoked.
struct TrackingAggregate {
    invoked: AtomicBool,
}

impl TrackingAggregate {
    fn new() -> Self {
        Self {
            invoked: AtomicBool::new(false),
        }
    }

    fn was_invoked(&self) -> bool {
        self.invoked.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl AggregateHandler for TrackingAggregate {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        self.invoked.store(true, Ordering::SeqCst);
        EchoAggregate::new().handle(ctx).await
    }
}

/// Wrapper for Arc<TrackingAggregate> to implement AggregateHandler.
struct TrackingAggregateWrapper(Arc<TrackingAggregate>);

#[async_trait]
impl AggregateHandler for TrackingAggregateWrapper {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        self.0.handle(ctx).await
    }
}

/// Aggregate that rejects commands based on state.
struct StatefulRejectAggregate;

#[async_trait]
impl AggregateHandler for StatefulRejectAggregate {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        // Check current state
        let event_count = ctx.events.as_ref().map(|e| e.pages.len()).unwrap_or(0);

        // Reject if aggregate already has events and command says "reject"
        let command_book = ctx.command.as_ref().unwrap();
        if let Some(page) = command_book.pages.first() {
            if let Some(cmd) = &page.command {
                if event_count > 0 && String::from_utf8_lossy(&cmd.value).contains("reject") {
                    return Err(Status::failed_precondition(
                        "Aggregate rejected due to state conflict",
                    ));
                }
            }
        }

        EchoAggregate::new().handle(ctx).await
    }
}

// ============================================================================
// MERGE_STRICT Tests
// ============================================================================

#[tokio::test]
async fn test_strict_correct_sequence_succeeds() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // First command at sequence 0
    let cmd = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeStrict);
    let result = client.execute(cmd).await;
    assert!(
        result.is_ok(),
        "STRICT with correct sequence should succeed"
    );

    // Verify event persisted
    let events = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .unwrap();
    assert_eq!(events.len(), 1, "Event should be persisted");
}

#[tokio::test]
async fn test_strict_stale_sequence_rejected() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // Create first event
    let cmd1 = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeStrict);
    client.execute(cmd1).await.expect("First command failed");

    // Try with stale sequence (0 instead of 1)
    let cmd2 = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeStrict);
    let result = client.execute(cmd2).await;

    assert!(result.is_err(), "STRICT with stale sequence should fail");
    let err = result.unwrap_err();
    // STRICT returns ABORTED (non-retryable) - check error message
    let err_msg = err.to_string().to_lowercase();
    assert!(
        err_msg.contains("aborted") || err_msg.contains("sequence"),
        "Should be ABORTED or mention sequence, got: {}",
        err
    );
}

#[tokio::test]
async fn test_strict_future_sequence_rejected() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // Try with future sequence (5 on new aggregate)
    let cmd = create_command_with_strategy("orders", root, 5, MergeStrategy::MergeStrict);
    let result = client.execute(cmd).await;

    assert!(result.is_err(), "STRICT with future sequence should fail");
}

#[tokio::test]
async fn test_strict_rejection_does_not_persist() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // First event
    let cmd1 = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeStrict);
    client.execute(cmd1).await.expect("First failed");

    // Rejected command
    let cmd2 = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeStrict);
    let _ = client.execute(cmd2).await;

    // Only first event persisted
    let events = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .unwrap();
    assert_eq!(events.len(), 1, "Rejected command should not persist");
}

// ============================================================================
// MERGE_COMMUTATIVE Tests
// ============================================================================

#[tokio::test]
async fn test_commutative_correct_sequence_succeeds() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    let cmd = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeCommutative);
    let result = client.execute(cmd).await;
    assert!(
        result.is_ok(),
        "COMMUTATIVE with correct sequence should succeed"
    );
}

#[tokio::test]
async fn test_commutative_stale_sequence_returns_retryable() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // Create first event
    let cmd1 = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeCommutative);
    client.execute(cmd1).await.expect("First command failed");

    // Try with stale sequence
    let cmd2 = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeCommutative);
    let result = client.execute(cmd2).await;

    assert!(
        result.is_err(),
        "COMMUTATIVE with stale sequence should fail"
    );
    let err = result.unwrap_err();
    // COMMUTATIVE returns FAILED_PRECONDITION (retryable) - check error message
    let err_msg = err.to_string().to_lowercase();
    assert!(
        err_msg.contains("failed_precondition") || err_msg.contains("sequence"),
        "Should be FAILED_PRECONDITION or mention sequence, got: {}",
        err
    );
}

#[tokio::test]
async fn test_commutative_default_when_unspecified() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // Create command without explicit strategy (proto default is 0 = COMMUTATIVE)
    let cmd = create_test_command("orders", root, b"test", 0);
    let result = client.execute(cmd).await;
    assert!(result.is_ok(), "Default strategy should work");

    // Verify it behaves as COMMUTATIVE by checking stale sequence returns retryable
    let cmd_stale = create_test_command("orders", root, b"test", 0);
    let result_stale = client.execute(cmd_stale).await;
    assert!(result_stale.is_err());
    let err = result_stale.unwrap_err();
    let err_msg = err.to_string().to_lowercase();
    assert!(
        err_msg.contains("failed_precondition") || err_msg.contains("sequence"),
        "Default should behave as COMMUTATIVE (FAILED_PRECONDITION), got: {}",
        err
    );
}

// ============================================================================
// MERGE_AGGREGATE_HANDLES Tests
// ============================================================================

#[tokio::test]
async fn test_aggregate_handles_bypasses_validation() {
    let aggregate = Arc::new(TrackingAggregate::new());
    let aggregate_clone = Arc::clone(&aggregate);

    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", TrackingAggregateWrapper(aggregate_clone))
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // First event at sequence 0
    let cmd1 =
        create_command_with_strategy("orders", root, 0, MergeStrategy::MergeAggregateHandles);
    client.execute(cmd1).await.expect("First failed");

    // Reset tracking
    aggregate.invoked.store(false, Ordering::SeqCst);

    // Second command at stale sequence 0 - should still invoke aggregate
    let cmd2 =
        create_command_with_strategy("orders", root, 0, MergeStrategy::MergeAggregateHandles);
    let result = client.execute(cmd2).await;

    // AGGREGATE_HANDLES passes through to aggregate
    assert!(
        aggregate.was_invoked(),
        "Aggregate should be invoked even with stale sequence"
    );
    assert!(
        result.is_ok(),
        "AGGREGATE_HANDLES should delegate to aggregate"
    );
}

#[tokio::test]
async fn test_aggregate_handles_aggregate_can_reject() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", StatefulRejectAggregate)
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // First event
    let cmd1 =
        create_command_with_strategy("orders", root, 0, MergeStrategy::MergeAggregateHandles);
    client.execute(cmd1).await.expect("First failed");

    // Second command that aggregate will reject based on state
    let mut cmd2 =
        create_command_with_strategy("orders", root, 0, MergeStrategy::MergeAggregateHandles);
    cmd2.pages[0].command = Some(Any {
        type_url: "test.RejectCommand".to_string(),
        value: b"reject".to_vec(),
    });

    let result = client.execute(cmd2).await;
    assert!(result.is_err(), "Aggregate should be able to reject");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("state conflict"),
        "Should have aggregate's rejection reason, got: {}",
        err
    );
}

#[tokio::test]
async fn test_aggregate_handles_sequence_zero_on_existing() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // Create 3 events
    for seq in 0..3 {
        let cmd =
            create_command_with_strategy("orders", root, seq, MergeStrategy::MergeAggregateHandles);
        client.execute(cmd).await.expect("Setup failed");
    }

    // Send command with sequence 0 - should still work
    let cmd = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeAggregateHandles);
    let result = client.execute(cmd).await;

    assert!(
        result.is_ok(),
        "AGGREGATE_HANDLES should allow any sequence"
    );

    // Verify 4 events now
    let events = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .unwrap();
    assert_eq!(events.len(), 4);
}

// ============================================================================
// Cross-Strategy Tests
// ============================================================================

#[tokio::test]
async fn test_different_strategies_same_domain() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // STRICT at seq 0
    let cmd1 = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeStrict);
    client.execute(cmd1).await.expect("STRICT failed");

    // COMMUTATIVE at seq 1
    let cmd2 = create_command_with_strategy("orders", root, 1, MergeStrategy::MergeCommutative);
    client.execute(cmd2).await.expect("COMMUTATIVE failed");

    // AGGREGATE_HANDLES at seq 2
    let cmd3 =
        create_command_with_strategy("orders", root, 2, MergeStrategy::MergeAggregateHandles);
    client
        .execute(cmd3)
        .await
        .expect("AGGREGATE_HANDLES failed");

    let events = runtime
        .event_store("orders")
        .unwrap()
        .get("orders", DEFAULT_EDITION, root)
        .await
        .unwrap();
    assert_eq!(events.len(), 3, "All three strategies should persist");
}

#[tokio::test]
async fn test_new_aggregate_all_strategies_accept_zero() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();

    for strategy in [
        MergeStrategy::MergeStrict,
        MergeStrategy::MergeCommutative,
        MergeStrategy::MergeAggregateHandles,
    ] {
        let root = Uuid::new_v4();
        let cmd = create_command_with_strategy("orders", root, 0, strategy);
        let result = client.execute(cmd).await;
        assert!(
            result.is_ok(),
            "New aggregate should accept seq 0 for {:?}",
            strategy
        );
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
async fn test_empty_pages_defaults_to_commutative() {
    // CommandBook with no pages should default to COMMUTATIVE behavior
    let cmd = CommandBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        saga_origin: None,
    };

    // Extract strategy should return COMMUTATIVE
    use angzarr::proto_ext::CommandBookExt;
    assert_eq!(
        cmd.merge_strategy(),
        MergeStrategy::MergeCommutative,
        "Empty pages should default to COMMUTATIVE"
    );
}
