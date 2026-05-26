//! Command execution abstraction.
//!
//! `CommandExecutor` sends commands to aggregates and classifies the outcome
//! via `grpc/`'s remote `AggregateCoordinatorServiceClient`.

pub mod grpc;

use async_trait::async_trait;
use tonic::Code;

use crate::proto::{CommandBook, CommandResponse, EventBook, SyncMode};

/// Outcome of executing a single command.
#[derive(Debug)]
pub enum CommandOutcome {
    /// Command executed successfully.
    Success(CommandResponse),
    /// Retryable error (transient, per `crate::utils::retry::is_retryable_status`).
    /// Contains error description and optionally the current aggregate state
    /// for optimized retry without refetching. Set for sequence-conflict
    /// `FailedPrecondition` and the 5xx-class transient codes from R2-15
    /// decision #2 (`Unavailable`, `DeadlineExceeded`, `ResourceExhausted`,
    /// `Internal`, `Unknown`, `DataLoss`, `Cancelled`).
    Retryable {
        reason: String,
        current_state: Option<EventBook>,
    },
    /// Non-retryable rejection. Carries the originating gRPC `Code` so
    /// downstream consumers (DLQ, compensation) can classify without
    /// re-parsing the message. See `crate::dlq::trigger::CodeDlqExt` for
    /// the canonical permanent/transient split.
    Rejected { code: Code, message: String },
}

/// Executes commands against aggregates.
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    /// Execute a command and classify the result.
    ///
    /// The `sync_mode` parameter controls:
    /// - `Unspecified`: Async execution (commands go to bus, results via notification)
    /// - `Simple`: Sync projectors only, sagas async, events published to bus
    /// - `Cascade`: Full sync chain (projectors + sagas sync, no bus publishing)
    async fn execute(&self, command: CommandBook, sync_mode: SyncMode) -> CommandOutcome;
}
