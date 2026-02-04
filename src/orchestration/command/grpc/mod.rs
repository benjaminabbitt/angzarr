//! gRPC command executor.
//!
//! Executes commands via remote `AggregateCoordinatorClient` per domain.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use crate::proto::CommandBook;
use crate::proto_ext::{correlated_request, CoverExt};
use crate::utils::retry::is_retryable_status;
use crate::utils::sequence_validator::extract_event_book_from_status;

use super::CommandExecutor;
use super::CommandOutcome;

/// Executes commands via gRPC `AggregateCoordinatorClient` per domain.
#[derive(Clone)]
pub struct GrpcCommandExecutor {
    clients:
        Arc<HashMap<String, Arc<Mutex<AggregateCoordinatorClient<tonic::transport::Channel>>>>>,
}

impl GrpcCommandExecutor {
    /// Create with domain -> gRPC client mapping.
    pub fn new(
        clients: HashMap<String, AggregateCoordinatorClient<tonic::transport::Channel>>,
    ) -> Self {
        let wrapped = clients
            .into_iter()
            .map(|(k, v)| (k, Arc::new(Mutex::new(v))))
            .collect();
        Self {
            clients: Arc::new(wrapped),
        }
    }

    /// Execute a command and return the raw gRPC result.
    ///
    /// Exposed for callers that need `Result` rather than `CommandOutcome`.
    pub async fn execute_raw(
        &self,
        command_book: CommandBook,
    ) -> Result<crate::proto::CommandResponse, tonic::Status> {
        let domain = command_book.domain();
        let correlation_id = command_book.correlation_id().to_string();

        let client = self.clients.get(domain).ok_or_else(|| {
            tonic::Status::not_found(format!("No aggregate registered for domain: {}", domain))
        })?;

        let mut client = client.lock().await;
        client
            .handle(correlated_request(command_book, &correlation_id))
            .await
            .map(|r| r.into_inner())
    }
}

#[async_trait]
impl CommandExecutor for GrpcCommandExecutor {
    async fn execute(&self, command: CommandBook) -> CommandOutcome {
        match self.execute_raw(command).await {
            Ok(response) => CommandOutcome::Success(response),
            Err(e) if is_retryable_status(&e) => {
                let current_state = extract_event_book_from_status(&e);
                CommandOutcome::Retryable {
                    reason: e.message().to_string(),
                    current_state,
                }
            }
            Err(e) => CommandOutcome::Rejected(e.message().to_string()),
        }
    }
}

/// Executes all commands via a single `AggregateCoordinatorClient`.
///
/// For deployments with a single aggregate sidecar handling all domains.
/// Does not route by domain — all commands go to the same client.
pub struct SingleClientExecutor {
    client: Arc<Mutex<AggregateCoordinatorClient<tonic::transport::Channel>>>,
}

impl SingleClientExecutor {
    /// Create with a single gRPC client.
    pub fn new(client: Arc<Mutex<AggregateCoordinatorClient<tonic::transport::Channel>>>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CommandExecutor for SingleClientExecutor {
    async fn execute(&self, command: CommandBook) -> CommandOutcome {
        let correlation_id = command.correlation_id().to_string();
        let mut client = self.client.lock().await;
        match client
            .handle(correlated_request(command, &correlation_id))
            .await
        {
            Ok(response) => CommandOutcome::Success(response.into_inner()),
            Err(e) if is_retryable_status(&e) => {
                let current_state = extract_event_book_from_status(&e);
                CommandOutcome::Retryable {
                    reason: e.message().to_string(),
                    current_state,
                }
            }
            Err(e) => CommandOutcome::Rejected(e.message().to_string()),
        }
    }
}
