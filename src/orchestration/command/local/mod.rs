//! Local (in-process) command executor.
//!
//! Executes commands via the standalone `CommandRouter` (direct handler calls).

use std::sync::Arc;

use async_trait::async_trait;

use crate::proto::CommandBook;
use crate::standalone::CommandRouter;
use crate::utils::retry::is_retryable_status;
use crate::utils::sequence_validator::extract_event_book_from_status;

use super::CommandExecutor;
use super::CommandOutcome;

/// Executes commands via in-process `CommandRouter`.
pub struct LocalCommandExecutor {
    router: Arc<CommandRouter>,
}

impl LocalCommandExecutor {
    /// Create with a reference to the standalone command router.
    pub fn new(router: Arc<CommandRouter>) -> Self {
        Self { router }
    }
}

#[async_trait]
impl CommandExecutor for LocalCommandExecutor {
    async fn execute(&self, command: CommandBook) -> CommandOutcome {
        match self.router.execute_command(command).await {
            Ok(response) => CommandOutcome::Success(response),
            Err(e) => classify_local_error(e),
        }
    }
}

/// Classify a local command execution error as retryable or rejected.
fn classify_local_error(e: Box<dyn std::error::Error + Send + Sync>) -> CommandOutcome {
    // Check for tonic::Status (sequence mismatch returns Aborted)
    if let Some(status) = e.downcast_ref::<tonic::Status>() {
        if is_retryable_status(status) {
            let current_state = extract_event_book_from_status(status);
            return CommandOutcome::Retryable {
                reason: status.message().to_string(),
                current_state,
            };
        }
    }
    CommandOutcome::Rejected(e.to_string())
}
