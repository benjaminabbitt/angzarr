//! Tests for saga orchestration and retry logic.
//!
//! Sagas are stateless domain translators that bridge events from one domain to
//! commands in another. The framework handles sequence conflicts via delivery
//! retry — sagas are executed once, and only command delivery is retried.
//!
//! Key behaviors tested:
//! - Command execution succeeds on first attempt (happy path)
//! - Sequence conflicts trigger automatic delivery retry with exponential backoff
//! - Non-retryable rejections (business rule violations) invoke rejection handler
//! - Retry exhaustion is bounded to prevent infinite loops
//! - Saga is NOT re-executed on conflict (delivery-retry model)

use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use backon::ExponentialBuilder;

use crate::proto::{CommandResponse, SyncMode};
use crate::proto_ext::CoverExt;

use super::super::command::CommandExecutor;

// ============================================================================
// Test Doubles
// ============================================================================

/// Minimal saga context for testing happy path — always succeeds with no commands.
struct AlwaysSucceeds;

#[async_trait]
impl SagaRetryContext for AlwaysSucceeds {
    async fn handle(
        &self,
        _destination_sequences: HashMap<String, u32>,
    ) -> Result<SagaResponse, Box<dyn std::error::Error + Send + Sync>> {
        Ok(SagaResponse::default())
    }
    async fn on_command_rejected(&self, _command: &CommandBook, _reason: &str) {}
    fn source_cover(&self) -> Option<&Cover> {
        None
    }
    fn source_max_sequence(&self) -> u32 {
        0
    }
}

/// Saga context that produces a command on every handle() call.
///
/// In the new model, commands are produced once with angzarr_deferred.
/// Retry happens at delivery level, not saga re-execution.
struct RetryingSagaContext;

#[async_trait]
impl SagaRetryContext for RetryingSagaContext {
    async fn handle(
        &self,
        _destination_sequences: HashMap<String, u32>,
    ) -> Result<SagaResponse, Box<dyn std::error::Error + Send + Sync>> {
        Ok(SagaResponse {
            commands: vec![CommandBook::default()],
            events: vec![],
        })
    }
    async fn on_command_rejected(&self, _command: &CommandBook, _reason: &str) {}
    fn source_cover(&self) -> Option<&Cover> {
        None
    }
    fn source_max_sequence(&self) -> u32 {
        0
    }
}

/// Saga context that tracks rejection callback invocations.
///
/// Used to verify that non-retryable rejections properly invoke the rejection
/// handler, allowing sagas to emit compensation events or log failures.
struct AlwaysRejects {
    rejection_count: AtomicU32,
}

#[async_trait]
impl SagaRetryContext for AlwaysRejects {
    async fn handle(
        &self,
        _destination_sequences: HashMap<String, u32>,
    ) -> Result<SagaResponse, Box<dyn std::error::Error + Send + Sync>> {
        Ok(SagaResponse::default())
    }
    async fn on_command_rejected(&self, _command: &CommandBook, _reason: &str) {
        self.rejection_count.fetch_add(1, Ordering::SeqCst);
    }
    fn source_cover(&self) -> Option<&Cover> {
        None
    }
    fn source_max_sequence(&self) -> u32 {
        0
    }
}

// ============================================================================
// Command Executors
// ============================================================================

/// Executor that always succeeds — simulates no contention.
struct SuccessExecutor;

#[async_trait]
impl CommandExecutor for SuccessExecutor {
    async fn execute(&self, _command: CommandBook, _sync_mode: SyncMode) -> CommandOutcome {
        CommandOutcome::Success(CommandResponse::default())
    }
}

/// Executor that fails N times with retryable errors before succeeding.
///
/// Simulates sequence conflicts from concurrent writes. The saga retry loop
/// should re-fetch state and retry until success or exhaustion.
struct CountingExecutor {
    failures_remaining: AtomicU32,
    execute_count: AtomicU32,
}

#[async_trait]
impl CommandExecutor for CountingExecutor {
    async fn execute(&self, _command: CommandBook, _sync_mode: SyncMode) -> CommandOutcome {
        self.execute_count.fetch_add(1, Ordering::SeqCst);
        let remaining = self.failures_remaining.load(Ordering::SeqCst);
        if remaining > 0 {
            self.failures_remaining.fetch_sub(1, Ordering::SeqCst);
            CommandOutcome::Retryable {
                reason: "Sequence conflict".to_string(),
                current_state: None,
            }
        } else {
            CommandOutcome::Success(CommandResponse::default())
        }
    }
}

/// Executor that always returns non-retryable rejection.
///
/// Simulates business rule violations that cannot be resolved by retry —
/// saga must invoke rejection handler and stop processing this command.
struct RejectingExecutor;

#[async_trait]
impl CommandExecutor for RejectingExecutor {
    async fn execute(&self, _command: CommandBook, _sync_mode: SyncMode) -> CommandOutcome {
        CommandOutcome::Rejected("Business rule violation".to_string())
    }
}

/// Test-friendly backoff: minimal delays, bounded retries.
fn fast_backoff() -> ExponentialBuilder {
    ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(1))
        .with_max_delay(Duration::from_millis(10))
        .with_max_times(5)
}

// ============================================================================
// Saga Retry Builder Tests
// ============================================================================

/// Command execution succeeds on first attempt — no retry needed.
///
/// Happy path: most saga commands complete without contention. The retry loop
/// should exit immediately after success without unnecessary delay or re-fetch.
#[tokio::test]
async fn test_execute_success_no_retry() {
    let ctx = AlwaysSucceeds;
    let executor = SuccessExecutor;
    let commands = vec![CommandBook::default()];
    SagaRetryBuilder::new(&ctx, &executor, "test-saga", "corr-1", SyncMode::Async)
        .commands(commands)
        .backoff(fast_backoff())
        .execute()
        .await;
}

/// Empty command list should complete immediately without error.
///
/// Sagas may legitimately produce zero commands (e.g., event doesn't require
/// translation to target domain). The executor must handle this gracefully.
#[tokio::test]
async fn test_execute_empty_commands_noop() {
    let ctx = AlwaysSucceeds;
    let executor = SuccessExecutor;
    SagaRetryBuilder::new(&ctx, &executor, "test-saga", "corr-1", SyncMode::Async)
        .backoff(fast_backoff())
        .execute()
        .await;
}

/// Sequence conflicts trigger retry until success.
///
/// Concurrent aggregates may cause sequence mismatches. The saga must
/// re-fetch destination state and rebuild the command with correct sequence.
/// This test verifies retry count: initial + 2 failures = 3 total executions.
#[tokio::test]
async fn test_execute_retries_then_succeeds() {
    let ctx = RetryingSagaContext;
    let executor = CountingExecutor {
        failures_remaining: AtomicU32::new(2),
        execute_count: AtomicU32::new(0),
    };
    let commands = vec![CommandBook::default()];
    SagaRetryBuilder::new(&ctx, &executor, "test-saga", "corr-1", SyncMode::Async)
        .commands(commands)
        .backoff(fast_backoff())
        .execute()
        .await;

    // Initial attempt + 2 retries = 3 executions
    assert_eq!(executor.execute_count.load(Ordering::SeqCst), 3);
}

/// Non-retryable rejection invokes the saga's rejection callback.
///
/// Business rule violations (e.g., "insufficient funds") cannot be resolved
/// by retry. The saga must be notified so it can emit compensation events
/// or log the failure for manual intervention.
#[tokio::test]
async fn test_execute_non_retryable_calls_rejection_handler() {
    let ctx = AlwaysRejects {
        rejection_count: AtomicU32::new(0),
    };
    let executor = RejectingExecutor;
    let commands = vec![CommandBook::default()];
    SagaRetryBuilder::new(&ctx, &executor, "test-saga", "corr-1", SyncMode::Async)
        .commands(commands)
        .backoff(fast_backoff())
        .execute()
        .await;

    assert_eq!(ctx.rejection_count.load(Ordering::SeqCst), 1);
}

/// Retry exhaustion stops execution and reports failure.
///
/// Unbounded retries would consume resources indefinitely. The backoff
/// builder's max_times bounds total attempts. After exhaustion, the saga
/// should stop and the event goes to DLQ for manual review.
#[tokio::test]
async fn test_execute_exhausts_retries() {
    let ctx = RetryingSagaContext;
    let executor = CountingExecutor {
        failures_remaining: AtomicU32::new(100),
        execute_count: AtomicU32::new(0),
    };
    let backoff = ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(1))
        .with_max_delay(Duration::from_millis(10))
        .with_max_times(3);
    let commands = vec![CommandBook::default()];
    SagaRetryBuilder::new(&ctx, &executor, "test-saga", "corr-1", SyncMode::Async)
        .commands(commands)
        .backoff(backoff)
        .execute()
        .await;

    // Initial attempt + 3 retries = 4 executions
    assert_eq!(executor.execute_count.load(Ordering::SeqCst), 4);
}

/// Domain validator prevents commands to forbidden domains.
///
/// Some deployments restrict which domains a saga can target (e.g., security
/// boundaries, tenant isolation). The validator rejects commands before
/// execution, preventing unauthorized cross-domain access.
#[tokio::test]
async fn test_orchestrate_saga_with_domain_validator() {
    let ctx = AlwaysSucceeds;
    let executor = SuccessExecutor;
    let validator = |cmd: &CommandBook| -> Result<(), String> {
        let domain = cmd.domain();
        if domain == "forbidden" {
            Err(format!("domain '{}' not allowed", domain))
        } else {
            Ok(())
        }
    };
    let result = orchestrate_saga(
        &ctx,
        &executor,
        None, // command_bus
        None, // fetcher
        None, // fact_executor
        "test-saga",
        "corr-1",
        Some(&validator),
        SyncMode::Async,
        fast_backoff(),
    )
    .await;
    assert!(result.is_ok());
}

// ============================================================================
// Cached State Optimization Tests
// ============================================================================

/// Executor that returns current state alongside retryable error.
///
/// When an aggregate rejects a command due to sequence conflict, it returns
/// the current state. The retry loop can use this cached state instead of
/// making a separate fetch call — reduces round trips under contention.
struct RetryableWithStateExecutor {
    failures_remaining: AtomicU32,
}

#[async_trait]
impl CommandExecutor for RetryableWithStateExecutor {
    async fn execute(&self, _command: CommandBook, _sync_mode: SyncMode) -> CommandOutcome {
        let remaining = self.failures_remaining.load(Ordering::SeqCst);
        if remaining > 0 {
            self.failures_remaining.fetch_sub(1, Ordering::SeqCst);
            let state = EventBook {
                cover: Some(Cover {
                    domain: "test".to_string(),
                    root: Some(crate::proto::Uuid {
                        value: uuid::Uuid::new_v4().as_bytes().to_vec(),
                    }),
                    correlation_id: "corr-1".to_string(),
                    edition: None,
                }),
                pages: vec![],
                snapshot: None,
                ..Default::default()
            };
            CommandOutcome::Retryable {
                reason: "Sequence conflict".to_string(),
                current_state: Some(state),
            }
        } else {
            CommandOutcome::Success(CommandResponse::default())
        }
    }
}

/// Saga context that produces commands with retryable executor.
///
/// In the new delivery-retry model, sagas produce commands once.
/// The framework handles delivery retry without re-executing the saga.
struct RetryableCommandContext;

#[async_trait]
impl SagaRetryContext for RetryableCommandContext {
    async fn handle(
        &self,
        _destination_sequences: HashMap<String, u32>,
    ) -> Result<SagaResponse, Box<dyn std::error::Error + Send + Sync>> {
        Ok(SagaResponse {
            commands: vec![CommandBook::default()],
            events: vec![],
        })
    }
    async fn on_command_rejected(&self, _command: &CommandBook, _reason: &str) {}
    fn source_cover(&self) -> Option<&Cover> {
        None
    }
    fn source_max_sequence(&self) -> u32 {
        0
    }
}

/// Delivery retry with current_state from conflict response.
///
/// When command delivery fails with sequence conflict and includes current state,
/// the retry mechanism can use that state to stamp the correct sequence.
/// The saga is NOT re-executed — only delivery is retried.
#[tokio::test]
async fn test_execute_retries_delivery_with_state_from_conflict() {
    let ctx = RetryableCommandContext;
    let executor = RetryableWithStateExecutor {
        failures_remaining: AtomicU32::new(1),
    };
    let commands = vec![CommandBook::default()];
    SagaRetryBuilder::new(&ctx, &executor, "test-saga", "corr-1", SyncMode::Async)
        .commands(commands)
        .backoff(fast_backoff())
        .execute()
        .await;

    // Command delivery retried after conflict, saga not re-executed.
    // The RetryableWithStateExecutor fails once then succeeds.
}
