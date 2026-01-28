//! Command execution abstraction.
//!
//! `CommandExecutor` sends commands to aggregates and classifies the outcome.
//! - `local/`: calls in-process `CommandRouter::execute_command()`
//! - `grpc/`: calls remote `AggregateCoordinatorClient` via gRPC

pub mod grpc;
#[cfg(feature = "sqlite")]
pub mod local;

use async_trait::async_trait;

use crate::proto::{CommandBook, CommandResponse, EventBook};

/// Outcome of executing a single command.
#[derive(Debug)]
pub enum CommandOutcome {
    /// Command executed successfully.
    Success(CommandResponse),
    /// Retryable error (sequence conflict).
    /// Contains error description and optionally the current aggregate state
    /// for optimized retry without refetching.
    Retryable {
        reason: String,
        current_state: Option<EventBook>,
    },
    /// Non-retryable rejection. Contains rejection reason.
    Rejected(String),
}

/// Executes commands against aggregates.
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    /// Execute a command and classify the result.
    async fn execute(&self, command: CommandBook) -> CommandOutcome;
}
