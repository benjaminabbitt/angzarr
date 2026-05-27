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
        _sync_mode: SyncMode,
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
        _sync_mode: SyncMode,
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
        _sync_mode: SyncMode,
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
        CommandOutcome::Rejected {
            code: tonic::Code::FailedPrecondition,
            message: "Business rule violation".to_string(),
        }
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
                    ext: None,
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
        _sync_mode: SyncMode,
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

// ============================================================================
// H-12: AngzarrDeferred-stamp rewrite must preserve per-command sync_mode
// ============================================================================
//
// Saga handlers may tag an emitted command's `PageHeader.sync_mode` to override
// the inherited flow mode (e.g. `Decision` when the accept/reject must surface
// synchronously). The AngzarrDeferred-stamp rewrite in `orchestrate_saga` at
// `saga/mod.rs:446` (existing-deferred branch) and `saga/mod.rs:460` (default
// branch) reconstructs `PageHeader { sync_mode: None, sequence_type: ... }`
// — clobbering the handler's override. PM's equivalent path was fixed at
// `process_manager/mod.rs:487` (`preserved_sync_mode`); saga was missed.

use crate::proto::{
    command_page::Payload as CmdPayload, page_header::SequenceType, AngzarrDeferredSequence,
    CommandPage, MergeStrategy, PageHeader,
};
use tokio::sync::Mutex as AsyncMutex;

/// Executor that captures each CommandBook it sees so the test can inspect the
/// rewritten page header that `orchestrate_saga` produced.
struct CapturingExecutor {
    seen: AsyncMutex<Vec<CommandBook>>,
}

impl CapturingExecutor {
    fn new() -> Self {
        Self {
            seen: AsyncMutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl CommandExecutor for CapturingExecutor {
    async fn execute(&self, command: CommandBook, _sync_mode: SyncMode) -> CommandOutcome {
        self.seen.lock().await.push(command);
        CommandOutcome::Success(CommandResponse::default())
    }
}

/// Saga context that emits a single command whose page header carries an
/// explicit `sync_mode` override plus an `angzarr_deferred` marker with
/// `source = None` — drives the line 446 fill-in branch of the rewrite.
struct SagaWithExistingDeferredAndSyncMode {
    override_mode: SyncMode,
}

#[async_trait]
impl SagaRetryContext for SagaWithExistingDeferredAndSyncMode {
    async fn handle(
        &self,
        _destination_sequences: HashMap<String, u32>,
        _sync_mode: SyncMode,
    ) -> Result<SagaResponse, Box<dyn std::error::Error + Send + Sync>> {
        let header = PageHeader {
            sync_mode: Some(self.override_mode as i32),
            sequence_type: Some(SequenceType::AngzarrDeferred(AngzarrDeferredSequence {
                source: None,
                source_seq: 7,
            })),
        };
        let page = CommandPage {
            header: Some(header),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
            payload: Some(CmdPayload::Command(prost_types::Any {
                type_url: "test.SagaCommand".to_string(),
                value: vec![],
            })),
        };
        let cover = Cover {
            domain: "inventory".to_string(),
            correlation_id: "corr-1".to_string(),
            ..Default::default()
        };
        Ok(SagaResponse {
            commands: vec![CommandBook {
                cover: Some(cover),
                pages: vec![page],
            }],
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

/// Saga context that emits a single command whose page header carries an
/// explicit `sync_mode` override but NO `sequence_type` — drives the line 460
/// default branch of the rewrite (saga handler didn't set angzarr_deferred).
struct SagaWithNoDeferredAndSyncMode {
    override_mode: SyncMode,
}

#[async_trait]
impl SagaRetryContext for SagaWithNoDeferredAndSyncMode {
    async fn handle(
        &self,
        _destination_sequences: HashMap<String, u32>,
        _sync_mode: SyncMode,
    ) -> Result<SagaResponse, Box<dyn std::error::Error + Send + Sync>> {
        let header = PageHeader {
            sync_mode: Some(self.override_mode as i32),
            sequence_type: None,
        };
        let page = CommandPage {
            header: Some(header),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
            payload: Some(CmdPayload::Command(prost_types::Any {
                type_url: "test.SagaCommand".to_string(),
                value: vec![],
            })),
        };
        let cover = Cover {
            domain: "inventory".to_string(),
            correlation_id: "corr-1".to_string(),
            ..Default::default()
        };
        Ok(SagaResponse {
            commands: vec![CommandBook {
                cover: Some(cover),
                pages: vec![page],
            }],
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

/// H-12: when a saga sets `angzarr_deferred` with `source = None` AND tags an
/// explicit per-command `sync_mode`, the rewrite that fills in the source must
/// preserve the explicit `sync_mode` (mirror of PM `preserved_sync_mode`).
///
/// Baseline reproduces the bug: rewrite emits `PageHeader { sync_mode: None,
/// ... }`, dropping the saga's override.
#[tokio::test]
async fn test_saga_rewrite_preserves_sync_mode_on_existing_deferred() {
    let ctx = SagaWithExistingDeferredAndSyncMode {
        override_mode: SyncMode::Decision,
    };
    let executor = CapturingExecutor::new();

    let result = orchestrate_saga(
        &ctx,
        &executor,
        None, // command_bus
        None, // fetcher
        None, // fact_executor
        "saga-h12-existing-deferred",
        "corr-1",
        None,
        SyncMode::Simple, // inherited mode
        fast_backoff(),
    )
    .await;
    assert!(result.is_ok(), "orchestrate_saga should succeed");

    let captured = executor.seen.lock().await;
    assert_eq!(
        captured.len(),
        1,
        "expected one CommandBook through executor"
    );
    let header = captured[0]
        .pages
        .first()
        .and_then(|p| p.header.as_ref())
        .expect("rewritten page should have a header");
    assert_eq!(
        header.sync_mode,
        Some(SyncMode::Decision as i32),
        "rewrite must preserve the saga handler's per-command sync_mode override \
         (existing-deferred branch at saga/mod.rs:446)"
    );
}

/// H-12: when a saga emits a command with NO `sequence_type` but an explicit
/// per-command `sync_mode`, the default-deferred rewrite branch must preserve
/// the explicit `sync_mode` (mirror of PM `preserved_sync_mode`).
///
/// Baseline reproduces the bug: rewrite emits `PageHeader { sync_mode: None,
/// ... }`, dropping the saga's override.
#[tokio::test]
async fn test_saga_rewrite_preserves_sync_mode_on_default_branch() {
    let ctx = SagaWithNoDeferredAndSyncMode {
        override_mode: SyncMode::Decision,
    };
    let executor = CapturingExecutor::new();

    let result = orchestrate_saga(
        &ctx,
        &executor,
        None, // command_bus
        None, // fetcher
        None, // fact_executor
        "saga-h12-default-branch",
        "corr-1",
        None,
        SyncMode::Simple,
        fast_backoff(),
    )
    .await;
    assert!(result.is_ok(), "orchestrate_saga should succeed");

    let captured = executor.seen.lock().await;
    assert_eq!(
        captured.len(),
        1,
        "expected one CommandBook through executor"
    );
    let header = captured[0]
        .pages
        .first()
        .and_then(|p| p.header.as_ref())
        .expect("rewritten page should have a header");
    assert_eq!(
        header.sync_mode,
        Some(SyncMode::Decision as i32),
        "rewrite must preserve the saga handler's per-command sync_mode override \
         (default-deferred branch at saga/mod.rs:460)"
    );
}

// ============================================================================
// H-15: fact_executor: None must not silently drop facts (saga side)
// ============================================================================
//
// `orchestrate_saga` at saga/mod.rs:507-524 has the same silent-drop bug as
// the PM coordinator: when `fact_executor: None` AND the SagaResponse carries
// facts (events), every fact is silently discarded. Doc-comments at the call
// site claim "facts are part of the transaction" but the API offers no
// enforcement. Mirror the PM fix: return Err so callers cannot accidentally
// regress the bc1d3db4 silent-drop class by forgetting to wire an executor.

/// Saga context that emits a single fact (`SagaResponse.events`) to drive
/// the H-15 saga-side fix.
struct SagaWithFact;

#[async_trait]
impl SagaRetryContext for SagaWithFact {
    async fn handle(
        &self,
        _destination_sequences: HashMap<String, u32>,
        _sync_mode: SyncMode,
    ) -> Result<SagaResponse, Box<dyn std::error::Error + Send + Sync>> {
        let fact = EventBook {
            cover: Some(Cover {
                domain: "inventory".to_string(),
                correlation_id: "corr-1".to_string(),
                ..Default::default()
            }),
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };
        Ok(SagaResponse {
            commands: vec![],
            events: vec![fact],
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

/// H-15 (saga side): saga emits facts but `fact_executor` is None — the
/// orchestrator must return Err rather than silently drop the facts. Mirror
/// of the PM-side test `test_orchestrate_pm_refuses_facts_without_fact_executor`.
#[tokio::test]
async fn test_orchestrate_saga_refuses_facts_without_fact_executor() {
    let ctx = SagaWithFact;
    let executor = SuccessExecutor;

    let result = orchestrate_saga(
        &ctx,
        &executor,
        None, // command_bus
        None, // fetcher
        None, // <-- no fact_executor; facts must NOT be silently dropped
        "test-saga",
        "corr-1",
        None,
        SyncMode::Async,
        fast_backoff(),
    )
    .await;

    assert!(
        result.is_err(),
        "Saga that emits facts with no fact_executor configured must return \
         Err — silent drop hides the bc1d3db4 regression class. Got Ok."
    );
    if let Err(e) = result {
        let msg = format!("{e}");
        assert!(
            msg.to_lowercase().contains("fact"),
            "saga error message must name 'fact' so operators can diagnose \
             the missing wiring. Got: {msg}"
        );
    }
}

// ============================================================================
// H-17: SagaRetryContext::handle must receive the inherited sync_mode
// ============================================================================

struct RecordingSyncModeContext {
    recorded: AsyncMutex<Option<SyncMode>>,
}

impl RecordingSyncModeContext {
    fn new() -> Self {
        Self {
            recorded: AsyncMutex::new(None),
        }
    }
}

#[async_trait]
impl SagaRetryContext for RecordingSyncModeContext {
    async fn handle(
        &self,
        _destination_sequences: HashMap<String, u32>,
        sync_mode: SyncMode,
    ) -> Result<SagaResponse, Box<dyn std::error::Error + Send + Sync>> {
        *self.recorded.lock().await = Some(sync_mode);
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

#[tokio::test]
async fn test_orchestrate_saga_threads_sync_mode_to_context_handle() {
    let ctx = RecordingSyncModeContext::new();
    let executor = SuccessExecutor;
    let result = orchestrate_saga(
        &ctx,
        &executor,
        None,
        None,
        None,
        "saga-h17",
        "corr-1",
        None,
        SyncMode::Decision,
        fast_backoff(),
    )
    .await;
    assert!(result.is_ok());
    let recorded = ctx.recorded.lock().await;
    assert_eq!(
        *recorded,
        Some(SyncMode::Decision),
        "H-17: orchestrate_saga must thread its sync_mode argument into SagaRetryContext::handle"
    );
}

// ============================================================================
// DLQ Wiring Tests (R2-15 step 5a)
// ============================================================================
//
// Saga has two DLQ sites:
//
// 1. Immediate-rejection: `CommandOutcome::Rejected` whose `tonic::Code`
//    classifies as `DlqTrigger::Immediate` (4xx-class). No retry happens;
//    DLQ entry is published from inside `try_execute`.
//
// 2. Retry-exhausted: `CommandOutcome::Retryable` (5xx-class transient
//    or sequence-conflict FailedPrecondition) where the backoff budget
//    is exhausted. DLQ entries are published from
//    `SagaRetryBuilder::execute` for every command in the final
//    attempt's failure set.
//
// The test fakes below capture published dead letters so each scenario
// can assert exactly which entries were emitted.

use crate::dlq::{AngzarrDeadLetter, DeadLetterPublisher, DlqError, RejectionDetails};
use async_trait::async_trait as test_async_trait;

/// Captures published dead letters for assertions.
struct CapturingDlqPublisher {
    captured: AsyncMutex<Vec<AngzarrDeadLetter>>,
}

impl CapturingDlqPublisher {
    fn new() -> Self {
        Self {
            captured: AsyncMutex::new(Vec::new()),
        }
    }
}

#[test_async_trait]
impl DeadLetterPublisher for CapturingDlqPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        self.captured.lock().await.push(dead_letter);
        Ok(())
    }
}

/// Saga context that wires a `dlq_publisher`. All other methods are
/// minimal — handle returns empty, source_cover returns None. The
/// DLQ-wiring tests construct commands and feed them through
/// `SagaRetryBuilder` directly, so the saga-handle path doesn't need
/// to do anything.
struct DlqAwareContext {
    publisher: Arc<dyn DeadLetterPublisher>,
    rejection_count: AtomicU32,
}

impl DlqAwareContext {
    fn new(publisher: Arc<dyn DeadLetterPublisher>) -> Self {
        Self {
            publisher,
            rejection_count: AtomicU32::new(0),
        }
    }
}

#[async_trait]
impl SagaRetryContext for DlqAwareContext {
    async fn handle(
        &self,
        _destination_sequences: HashMap<String, u32>,
        _sync_mode: SyncMode,
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
    fn dlq_publisher(&self) -> Option<&Arc<dyn DeadLetterPublisher>> {
        Some(&self.publisher)
    }
    fn component_name(&self) -> &str {
        "saga-test"
    }
}

/// Executor that always rejects with a given `tonic::Code`.
struct CodeRejectingExecutor {
    code: tonic::Code,
    message: String,
}

#[async_trait]
impl CommandExecutor for CodeRejectingExecutor {
    async fn execute(&self, _command: CommandBook, _sync_mode: SyncMode) -> CommandOutcome {
        CommandOutcome::Rejected {
            code: self.code,
            message: self.message.clone(),
        }
    }
}

/// Executor that always returns Retryable. Forces retry-exhaustion.
struct AlwaysRetryableExecutor {
    reason: String,
}

#[async_trait]
impl CommandExecutor for AlwaysRetryableExecutor {
    async fn execute(&self, _command: CommandBook, _sync_mode: SyncMode) -> CommandOutcome {
        CommandOutcome::Retryable {
            reason: self.reason.clone(),
            current_state: None,
        }
    }
}

/// 4xx-class command rejection publishes a dead letter immediately.
///
/// `InvalidArgument` is a permanent error per
/// `CodeDlqExt::classify_for_dlq` → `Immediate`. The saga must publish
/// a DLQ entry from inside `try_execute` (not wait for retry exhaustion)
/// AND still invoke `on_command_rejected` for compensation. This is
/// the R2-15 immediate-DLQ contract for sagas.
#[tokio::test]
async fn saga_4xx_command_rejection_publishes_dead_letter_immediately() {
    let publisher = Arc::new(CapturingDlqPublisher::new());
    let ctx = DlqAwareContext::new(publisher.clone());
    let executor = CodeRejectingExecutor {
        code: tonic::Code::InvalidArgument,
        message: "schema mismatch".to_string(),
    };
    let commands = vec![CommandBook::default()];

    SagaRetryBuilder::new(&ctx, &executor, "test-saga", "corr-1", SyncMode::Simple)
        .commands(commands)
        .backoff(fast_backoff())
        .execute()
        .await;

    // Compensation still runs (existing contract).
    assert_eq!(
        ctx.rejection_count.load(Ordering::SeqCst),
        1,
        "on_command_rejected must fire alongside the DLQ publish"
    );

    // Exactly one DLQ entry was published with the right shape.
    let captured = publisher.captured.lock().await;
    assert_eq!(
        captured.len(),
        1,
        "expected one immediate-rejection DLQ entry"
    );
    let dl = &captured[0];
    assert_eq!(dl.source_component, "saga-test");
    assert_eq!(dl.source_component_type, "saga");
    match &dl.rejection_details {
        Some(RejectionDetails::EventProcessingFailed(details)) => {
            assert_eq!(
                details.retry_count, 0,
                "immediate path: zero retries attempted"
            );
            assert!(!details.is_transient, "4xx is permanent");
            assert!(details.error.contains("schema mismatch"));
        }
        other => panic!("expected EventProcessingFailed, got {other:?}"),
    }
}

/// 5xx-class transient failure retries until exhausted, then publishes DLQ.
///
/// `Unavailable` is transient per `classify_for_dlq` → `RetryThenDlq`.
/// The framework's broadened `is_retryable_status` routes it into the
/// retry loop. When the backoff budget is exhausted, the saga must
/// publish a DLQ entry per failed command from
/// `SagaRetryBuilder::execute`. This is the R2-15 retry-then-DLQ
/// contract for sagas.
///
/// Note: because the gRPC `CommandExecutor` is what translates a
/// `tonic::Status` into either `Retryable` or `Rejected` (via
/// `is_retryable_status`), this test fakes the executor directly with
/// `Retryable` to exercise the retry-exhausted DLQ path without
/// spinning up a transport.
#[tokio::test]
async fn saga_5xx_command_rejection_retries_then_publishes_dead_letter() {
    let publisher = Arc::new(CapturingDlqPublisher::new());
    let ctx = DlqAwareContext::new(publisher.clone());
    let executor = AlwaysRetryableExecutor {
        reason: "Unavailable: broker down".to_string(),
    };
    let commands = vec![CommandBook::default()];

    SagaRetryBuilder::new(&ctx, &executor, "test-saga", "corr-1", SyncMode::Simple)
        .commands(commands)
        .backoff(fast_backoff())
        .execute()
        .await;

    // No compensation: retries exhausted is NOT a permanent business
    // rejection in the saga's mental model, so on_command_rejected is
    // not invoked (only Rejected outcomes invoke it). Verify that.
    assert_eq!(
        ctx.rejection_count.load(Ordering::SeqCst),
        0,
        "retry exhaustion does not invoke on_command_rejected"
    );

    // Exactly one DLQ entry was published for the single command that
    // failed on the final attempt.
    let captured = publisher.captured.lock().await;
    assert_eq!(captured.len(), 1, "expected one retry-exhausted DLQ entry");
    let dl = &captured[0];
    assert_eq!(dl.source_component, "saga-test");
    assert_eq!(dl.source_component_type, "saga");
    match &dl.rejection_details {
        Some(RejectionDetails::EventProcessingFailed(details)) => {
            assert!(
                details.retry_count > 0,
                "retry-exhausted path: attempts > 0, got {}",
                details.retry_count
            );
            assert!(details.is_transient, "5xx is transient");
            assert!(details.error.contains("Unavailable"));
        }
        other => panic!("expected EventProcessingFailed, got {other:?}"),
    }
}

/// Successful command execution publishes no dead letter.
///
/// Pins the "no false positives" half of the contract: the DLQ wiring
/// must not emit entries on the happy path. Otherwise operators would
/// be flooded by every successful saga.
#[tokio::test]
async fn saga_2xx_success_does_not_publish() {
    let publisher = Arc::new(CapturingDlqPublisher::new());
    let ctx = DlqAwareContext::new(publisher.clone());
    let executor = SuccessExecutor;
    let commands = vec![CommandBook::default()];

    SagaRetryBuilder::new(&ctx, &executor, "test-saga", "corr-1", SyncMode::Simple)
        .commands(commands)
        .backoff(fast_backoff())
        .execute()
        .await;

    let captured = publisher.captured.lock().await;
    assert!(
        captured.is_empty(),
        "success path must not publish any dead letters, got {} entries",
        captured.len()
    );
}
