//! MergeStrategy (concurrency control) integration tests.
//!
//! Tests the aggregate coordinator's handling of sequence conflicts based on
//! the MergeStrategy field in CommandPage.
//!
//! See also: examples/features/unit/merge_strategy.feature for Gherkin specs.

use crate::common::*;
use angzarr::proto::{event_page, EventPage, MergeStrategy};
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
            external_payload: None,
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
    // STRICT returns FAILED_PRECONDITION (retryable) for update-and-retry flow
    let err_msg = err.to_string().to_lowercase();
    assert!(
        err_msg.contains("failed_precondition") || err_msg.contains("sequence"),
        "Should be FAILED_PRECONDITION or mention sequence, got: {}",
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

// ============================================================================
// MERGE_STRICT Behavior Correction Tests
// ============================================================================
//
// STRICT should return FAILED_PRECONDITION (retryable) on sequence mismatch,
// NOT ABORTED (non-retryable). This allows the saga retry loop to reload
// fresh state and retry the command.

#[tokio::test]
async fn test_strict_stale_sequence_returns_failed_precondition() {
    // STRICT with stale sequence should return FAILED_PRECONDITION (retryable)
    // to enable update-and-retry flow.
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

    // STRICT should return FAILED_PRECONDITION (retryable), not ABORTED
    // This is the correct behavior for update-and-retry flow
    let err_msg = err.to_string().to_lowercase();
    // Check for "failedprecondition" (tonic debug format) or "failed_precondition"
    assert!(
        err_msg.contains("failedprecondition") || err_msg.contains("failed_precondition"),
        "STRICT should return FAILED_PRECONDITION for update-and-retry, got: {}",
        err
    );
    // Also verify it's NOT ABORTED
    assert!(
        !err_msg.contains("aborted"),
        "STRICT should NOT return ABORTED, got: {}",
        err
    );
}

#[tokio::test]
async fn test_strict_is_retryable() {
    // Verify that STRICT sequence mismatch is treated as retryable by the retry logic
    use angzarr::utils::retry::is_retryable_status;
    use tonic::Status;

    // Simulated STRICT rejection - should be retryable
    let status =
        Status::failed_precondition("Sequence mismatch: command expects 0, aggregate at 1");
    assert!(
        is_retryable_status(&status),
        "STRICT sequence mismatch should be retryable"
    );
}

// ============================================================================
// MERGE_MANUAL Tests (DLQ Routing)
// ============================================================================
//
// MERGE_MANUAL sends sequence mismatches directly to DLQ for human review.
// No automatic retry, no commutative merge attempt.

#[tokio::test]
async fn test_manual_correct_sequence_succeeds() {
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    let cmd = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeManual);
    let result = client.execute(cmd).await;
    assert!(
        result.is_ok(),
        "MANUAL with correct sequence should succeed"
    );
}

#[tokio::test]
async fn test_manual_stale_sequence_sends_to_dlq() {
    // MANUAL should send to DLQ and return ABORTED on sequence mismatch
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // Create first event
    let cmd1 = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeManual);
    client.execute(cmd1).await.expect("First command failed");

    // Try with stale sequence
    let cmd2 = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeManual);
    let result = client.execute(cmd2).await;

    assert!(result.is_err(), "MANUAL with stale sequence should fail");
    let err = result.unwrap_err();

    // MANUAL returns ABORTED (non-retryable) - it went to DLQ
    let err_msg = err.to_string().to_lowercase();
    assert!(
        err_msg.contains("aborted") || err_msg.contains("dlq"),
        "MANUAL should return ABORTED and mention DLQ, got: {}",
        err
    );
}

#[tokio::test]
async fn test_manual_is_not_retryable() {
    // Verify that MANUAL DLQ routing is NOT retryable
    use angzarr::utils::retry::is_retryable_status;
    use tonic::Status;

    // MANUAL sends to DLQ and returns ABORTED - NOT retryable
    let status = Status::aborted("Sent to DLQ for manual review");
    assert!(
        !is_retryable_status(&status),
        "MANUAL DLQ routing should NOT be retryable"
    );
}

// ============================================================================
// MERGE_COMMUTATIVE Field-Level Merge Tests
// ============================================================================
//
// COMMUTATIVE should attempt field-level merge when sequences don't match:
// 1. Replay state at expected sequence (N)
// 2. Replay state at actual sequence (M)
// 3. Diff state changes from N→M (intervening changes)
// 4. Dry-run command, get resulting events, apply to get command changes
// 5. If field changes are disjoint → allow (commutative)
// 6. If field changes overlap → return FAILED_PRECONDITION (retry)

/// Test aggregate that tracks state with multiple fields for commutative merge testing.
///
/// Commands:
/// - "update_field_a:<value>" - Updates field_a
/// - "update_field_b:<value>" - Updates field_b
/// - "update_both:<a>:<b>" - Updates both fields
///
/// The aggregate implements Replay to return state as Any for field diff detection.
struct StatefulAggregate;

impl StatefulAggregate {
    fn new() -> Self {
        Self
    }

    /// Rebuild state from events.
    fn rebuild_state(events: &EventBook) -> (i32, String) {
        let mut field_a: i32 = 0;
        let mut field_b: String = String::new();

        for page in &events.pages {
            if let Some(event) = &page.event {
                let value = String::from_utf8_lossy(&event.value);
                if event.type_url.contains("FieldAUpdated") {
                    field_a = value.parse().unwrap_or(0);
                } else if event.type_url.contains("FieldBUpdated") {
                    field_b = value.to_string();
                }
            }
        }

        (field_a, field_b)
    }

    /// Pack state as Any for Replay RPC.
    /// Uses a simple JSON-like encoding for test purposes.
    fn pack_state(field_a: i32, field_b: &str) -> prost_types::Any {
        // For testing, encode as JSON-like bytes with type URL
        // In production, this would use proper proto encoding
        let value = format!(r#"{{"field_a":{},"field_b":"{}"}}"#, field_a, field_b);
        prost_types::Any {
            type_url: "test.StatefulState".to_string(),
            value: value.into_bytes(),
        }
    }
}

#[async_trait]
impl AggregateHandler for StatefulAggregate {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        let command_book = ctx
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command"))?;

        let cover = command_book.cover.clone();

        // Get next sequence from prior events
        let next_seq = ctx
            .events
            .as_ref()
            .and_then(|e| e.pages.last())
            .and_then(|p| match &p.sequence {
                Some(event_page::Sequence::Num(n)) => Some(n + 1),
                _ => None,
            })
            .unwrap_or(0);

        // Parse command and emit corresponding event
        let mut event_pages = Vec::new();
        for cmd_page in &command_book.pages {
            if let Some(cmd) = &cmd_page.command {
                let cmd_value = String::from_utf8_lossy(&cmd.value);

                if cmd_value.starts_with("update_field_a:") {
                    let value = cmd_value.strip_prefix("update_field_a:").unwrap_or("0");
                    event_pages.push(EventPage {
                        sequence: Some(event_page::Sequence::Num(
                            next_seq + event_pages.len() as u32,
                        )),
                        event: Some(Any {
                            type_url: "test.FieldAUpdated".to_string(),
                            value: value.as_bytes().to_vec(),
                        }),
                        created_at: None,
                        external_payload: None,
                    });
                } else if cmd_value.starts_with("update_field_b:") {
                    let value = cmd_value.strip_prefix("update_field_b:").unwrap_or("");
                    event_pages.push(EventPage {
                        sequence: Some(event_page::Sequence::Num(
                            next_seq + event_pages.len() as u32,
                        )),
                        event: Some(Any {
                            type_url: "test.FieldBUpdated".to_string(),
                            value: value.as_bytes().to_vec(),
                        }),
                        created_at: None,
                        external_payload: None,
                    });
                } else if cmd_value.starts_with("update_both:") {
                    let parts: Vec<&str> = cmd_value
                        .strip_prefix("update_both:")
                        .unwrap_or("0:default")
                        .split(':')
                        .collect();
                    let a_val = parts.first().unwrap_or(&"0");
                    let b_val = parts.get(1).unwrap_or(&"default");

                    event_pages.push(EventPage {
                        sequence: Some(event_page::Sequence::Num(
                            next_seq + event_pages.len() as u32,
                        )),
                        event: Some(Any {
                            type_url: "test.FieldAUpdated".to_string(),
                            value: a_val.as_bytes().to_vec(),
                        }),
                        created_at: None,
                        external_payload: None,
                    });
                    event_pages.push(EventPage {
                        sequence: Some(event_page::Sequence::Num(
                            next_seq + event_pages.len() as u32,
                        )),
                        event: Some(Any {
                            type_url: "test.FieldBUpdated".to_string(),
                            value: b_val.as_bytes().to_vec(),
                        }),
                        created_at: None,
                        external_payload: None,
                    });
                } else {
                    // Default: echo as-is
                    event_pages.push(EventPage {
                        sequence: Some(event_page::Sequence::Num(
                            next_seq + event_pages.len() as u32,
                        )),
                        event: cmd_page.command.clone(),
                        created_at: None,
                        external_payload: None,
                    });
                }
            }
        }

        Ok(EventBook {
            cover,
            pages: event_pages,
            snapshot: None,
            ..Default::default()
        })
    }

    async fn replay(&self, events: &EventBook) -> Result<prost_types::Any, Status> {
        let (field_a, field_b) = Self::rebuild_state(events);
        Ok(Self::pack_state(field_a, &field_b))
    }
}

/// Create a command that updates field_a.
fn create_field_a_command(domain: &str, root: Uuid, sequence: u32, value: i32) -> CommandBook {
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
                type_url: "test.UpdateFieldA".to_string(),
                value: format!("update_field_a:{}", value).into_bytes(),
            }),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
            external_payload: None,
        }],
        saga_origin: None,
    }
}

/// Create a command that updates field_b.
fn create_field_b_command(domain: &str, root: Uuid, sequence: u32, value: &str) -> CommandBook {
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
                type_url: "test.UpdateFieldB".to_string(),
                value: format!("update_field_b:{}", value).into_bytes(),
            }),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
            external_payload: None,
        }],
        saga_origin: None,
    }
}

#[tokio::test]
async fn test_commutative_disjoint_fields_succeeds() {
    // Two commands that modify different fields should both succeed
    // even with stale sequences, because their changes are commutative.
    //
    // Scenario:
    // 1. Command A modifies field_a (sequence 0)
    // 2. Command B modifies field_b (sequence 0 - stale!)
    // 3. Because field_a and field_b are disjoint, B should succeed

    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("stateful", StatefulAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // First command: update field_a at sequence 0
    let cmd_a = create_field_a_command("stateful", root, 0, 100);
    client
        .execute(cmd_a)
        .await
        .expect("First command (field_a) failed");

    // Second command: update field_b at stale sequence 0 (should be 1)
    // This modifies a DIFFERENT field, so it should be allowed (commutative)
    let cmd_b = create_field_b_command("stateful", root, 0, "hello");
    let result = client.execute(cmd_b).await;

    // EXPECTED: Should succeed because fields are disjoint
    // CURRENT: Will fail because COMMUTATIVE logic isn't implemented yet
    assert!(
        result.is_ok(),
        "Disjoint field changes should succeed with COMMUTATIVE strategy. Got: {:?}",
        result.err()
    );

    // Verify both events persisted
    let events = runtime
        .event_store("stateful")
        .unwrap()
        .get("stateful", DEFAULT_EDITION, root)
        .await
        .unwrap();
    assert_eq!(events.len(), 2, "Both events should be persisted");
}

#[tokio::test]
async fn test_commutative_overlapping_fields_returns_retryable() {
    // Two commands that modify the same field should fail with
    // FAILED_PRECONDITION when sequences don't match.
    //
    // Scenario:
    // 1. Command A modifies field_a (sequence 0)
    // 2. Command B also modifies field_a (sequence 0 - stale!)
    // 3. Because both touch field_a, B should fail with retryable error

    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("stateful", StatefulAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // First command: update field_a at sequence 0
    let cmd_a = create_field_a_command("stateful", root, 0, 100);
    client.execute(cmd_a).await.expect("First command failed");

    // Second command: also update field_a at stale sequence 0
    // This modifies the SAME field, so it should fail
    let cmd_b = create_field_a_command("stateful", root, 0, 200);
    let result = client.execute(cmd_b).await;

    // EXPECTED: Should fail with FAILED_PRECONDITION (retryable)
    assert!(result.is_err(), "Overlapping field changes should fail");
    let err = result.unwrap_err();
    let err_msg = err.to_string().to_lowercase();
    assert!(
        err_msg.contains("failedprecondition") || err_msg.contains("failed_precondition"),
        "Should return FAILED_PRECONDITION for overlapping fields, got: {}",
        err
    );
}

#[tokio::test]
async fn test_commutative_falls_back_to_retry_when_replay_unavailable() {
    // When aggregate doesn't implement Replay RPC, COMMUTATIVE should
    // degrade gracefully to STRICT-like behavior (retry).
    //
    // This ensures backwards compatibility with aggregates that don't
    // implement Replay.

    // Use EchoAggregate which doesn't implement Replay (returns Unimplemented)
    let runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .build()
        .await
        .expect("Failed to build runtime");

    let client = runtime.command_client();
    let root = Uuid::new_v4();

    // First command at sequence 0
    let cmd1 = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeCommutative);
    client.execute(cmd1).await.expect("First command failed");

    // Second command at stale sequence 0
    let cmd2 = create_command_with_strategy("orders", root, 0, MergeStrategy::MergeCommutative);
    let result = client.execute(cmd2).await;

    // Should fail with FAILED_PRECONDITION because Replay unavailable → degrade to STRICT
    assert!(result.is_err(), "Should fail when Replay unavailable");
    let err = result.unwrap_err();
    let err_msg = err.to_string().to_lowercase();
    assert!(
        err_msg.contains("failedprecondition")
            || err_msg.contains("failed_precondition")
            || err_msg.contains("sequence"),
        "Should return FAILED_PRECONDITION when Replay unavailable, got: {}",
        err
    );
}
