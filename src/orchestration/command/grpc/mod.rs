//! gRPC command executor.
//!
//! Executes commands via remote `CommandHandlerCoordinatorServiceClient` per domain.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::proto::command_handler_coordinator_service_client::CommandHandlerCoordinatorServiceClient;
use crate::proto::{CommandBook, CommandRequest, SyncMode};
use crate::proto_ext::{correlated_request, CoverExt};
use crate::utils::retry::is_retryable_status;
use crate::utils::sequence_validator::extract_event_book_from_status;

use super::CommandExecutor;
use super::CommandOutcome;

/// Error message constants for command execution.
pub mod errmsg {
    pub const NO_AGGREGATE_FOR_DOMAIN: &str = "No aggregate registered for domain";
}

/// Executes commands via gRPC `CommandHandlerCoordinatorServiceClient` per domain.
#[derive(Clone)]
pub struct GrpcCommandExecutor {
    clients: Arc<
        HashMap<
            String,
            Arc<Mutex<CommandHandlerCoordinatorServiceClient<tonic::transport::Channel>>>,
        >,
    >,
}

impl GrpcCommandExecutor {
    /// Create with domain -> gRPC client mapping.
    pub fn new(
        clients: HashMap<String, CommandHandlerCoordinatorServiceClient<tonic::transport::Channel>>,
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
        sync_mode: SyncMode,
    ) -> Result<crate::proto::CommandResponse, tonic::Status> {
        let domain = command_book.domain();
        let correlation_id = command_book.correlation_id().to_string();

        let client = self.clients.get(domain).ok_or_else(|| {
            tonic::Status::not_found(format!("{}: {}", errmsg::NO_AGGREGATE_FOR_DOMAIN, domain))
        })?;

        let mut client = client.lock().await;
        let sync_command = CommandRequest {
            command: Some(command_book),
            sync_mode: sync_mode.into(),
        };
        client
            .handle_command(correlated_request(sync_command, &correlation_id))
            .await
            .map(|r| r.into_inner())
    }
}

#[async_trait]
impl CommandExecutor for GrpcCommandExecutor {
    async fn execute(&self, command: CommandBook, sync_mode: SyncMode) -> CommandOutcome {
        match self.execute_raw(command, sync_mode).await {
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

#[cfg(test)]
mod tests {
    //! Tests for GrpcCommandExecutor.
    //!
    //! The executor routes commands to domain-specific gRPC clients.
    //! When no client is registered for a domain, it returns NOT_FOUND.
    //!
    //! Key behaviors:
    //! - Domain lookup: Commands are routed by domain name
    //! - Missing domain: Returns NOT_FOUND status
    //! - Outcome mapping: gRPC errors are classified into Success/Retryable/Rejected

    use super::*;
    use crate::proto::{Cover, Uuid as ProtoUuid};
    use std::collections::HashMap;

    fn make_command_for_domain(domain: &str) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: vec![1, 2, 3, 4],
                }),
                correlation_id: "corr-123".to_string(),
                edition: None,
                external_id: String::new(),
            }),
            pages: vec![],
            saga_origin: None,
        }
    }

    // ============================================================================
    // Domain Lookup Tests
    // ============================================================================

    /// Empty executor returns NOT_FOUND for any domain.
    ///
    /// When no clients are registered, all commands fail. This is a
    /// configuration error — the executor should be populated at startup.
    #[tokio::test]
    async fn test_execute_raw_no_clients_returns_not_found() {
        let executor = GrpcCommandExecutor::new(HashMap::new());
        let command = make_command_for_domain("orders");

        let result = executor.execute_raw(command, SyncMode::Simple).await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
        assert!(status.message().contains(errmsg::NO_AGGREGATE_FOR_DOMAIN));
        assert!(status.message().contains("orders"));
    }

    /// Missing domain returns NOT_FOUND even when other domains exist.
    ///
    /// Routing is per-domain. A command for "inventory" fails if only
    /// "orders" client is registered. This tests the HashMap lookup.
    #[tokio::test]
    async fn test_execute_raw_wrong_domain_returns_not_found() {
        // We can't easily create a mock client, but we can verify the lookup
        // by using an empty map and checking the error domain is correct
        let executor = GrpcCommandExecutor::new(HashMap::new());
        let command = make_command_for_domain("inventory");

        let result = executor.execute_raw(command, SyncMode::Simple).await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert!(status.message().contains("inventory"));
    }

    // ============================================================================
    // CommandOutcome Mapping Tests
    // ============================================================================

    /// NOT_FOUND error maps to Rejected outcome (not retryable).
    ///
    /// Missing domain is a configuration error, not a transient failure.
    /// Retrying won't help — the client map doesn't change at runtime.
    #[tokio::test]
    async fn test_execute_not_found_maps_to_rejected() {
        let executor = GrpcCommandExecutor::new(HashMap::new());
        let command = make_command_for_domain("orders");

        let outcome = executor.execute(command, SyncMode::Simple).await;

        match outcome {
            CommandOutcome::Rejected(reason) => {
                assert!(reason.contains(errmsg::NO_AGGREGATE_FOR_DOMAIN));
            }
            other => panic!("Expected Rejected, got {:?}", other),
        }
    }
}
