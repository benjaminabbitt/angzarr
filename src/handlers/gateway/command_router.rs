//! Command routing for gateway service.
//!
//! Handles forwarding commands to aggregate services based on domain routing,
//! using K8s label-based service discovery.

use std::sync::Arc;

use tonic::Status;
use tracing::{debug, warn};

use crate::discovery::{DiscoveryError, ServiceDiscovery};
use crate::proto::{CommandBook, CommandResponse, DryRunRequest, SyncCommandBook};
use crate::proto_ext::{correlated_request, CoverExt, WILDCARD_DOMAIN};

/// Command router for forwarding commands to aggregate services.
#[derive(Clone)]
pub struct CommandRouter {
    discovery: Arc<dyn ServiceDiscovery>,
}

impl CommandRouter {
    /// Create a new command router with service discovery.
    pub fn new(discovery: Arc<dyn ServiceDiscovery>) -> Self {
        Self { discovery }
    }

    /// Extract existing correlation ID from command. Does not auto-generate.
    ///
    /// Correlation ID is client-provided for cross-domain workflows.
    /// If not provided, returns empty string (PMs won't trigger).
    #[allow(clippy::result_large_err)]
    pub fn ensure_correlation_id(command_book: &mut CommandBook) -> Result<String, Status> {
        Ok(command_book.correlation_id().to_string())
    }

    /// Forward command to aggregate coordinator based on domain.
    pub async fn forward_command(
        &self,
        command_book: CommandBook,
        correlation_id: &str,
    ) -> Result<CommandResponse, Status> {
        let domain = command_book
            .cover
            .as_ref()
            .map(|c| c.domain.clone())
            .unwrap_or_else(|| WILDCARD_DOMAIN.to_string());

        debug!(
            %domain,
            "Routing command to domain"
        );

        let mut client = self
            .discovery
            .get_aggregate(&domain)
            .await
            .map_err(map_discovery_error)?;

        client
            .handle(correlated_request(command_book, correlation_id))
            .await
            .map(|r| r.into_inner())
            .map_err(|e| {
                warn!(
                    %domain,
                    error = %e,
                    "Command failed"
                );
                e
            })
    }

    /// Forward dry-run request to aggregate coordinator based on domain.
    pub async fn forward_dry_run(
        &self,
        dry_run_request: DryRunRequest,
        correlation_id: &str,
    ) -> Result<CommandResponse, Status> {
        let domain = dry_run_request
            .command
            .as_ref()
            .and_then(|c| c.cover.as_ref())
            .map(|c| c.domain.clone())
            .unwrap_or_else(|| WILDCARD_DOMAIN.to_string());

        debug!(
            %domain,
            "Routing dry-run to domain"
        );

        let mut client = self
            .discovery
            .get_aggregate(&domain)
            .await
            .map_err(map_discovery_error)?;

        client
            .dry_run_handle(correlated_request(dry_run_request, correlation_id))
            .await
            .map(|r| r.into_inner())
            .map_err(|e| {
                warn!(
                    %domain,
                    error = %e,
                    "Dry-run command failed"
                );
                e
            })
    }

    /// Forward command synchronously to aggregate coordinator based on domain.
    pub async fn forward_command_sync(
        &self,
        command_book: CommandBook,
        sync_mode: i32,
        correlation_id: &str,
    ) -> Result<CommandResponse, Status> {
        let domain = command_book
            .cover
            .as_ref()
            .map(|c| c.domain.clone())
            .unwrap_or_else(|| WILDCARD_DOMAIN.to_string());

        debug!(
            %domain,
            %sync_mode,
            "Routing command to domain (sync)"
        );

        let mut client = self
            .discovery
            .get_aggregate(&domain)
            .await
            .map_err(map_discovery_error)?;

        let sync_request = SyncCommandBook {
            command: Some(command_book),
            sync_mode,
        };

        client
            .handle_sync(correlated_request(sync_request, correlation_id))
            .await
            .map(|r| r.into_inner())
            .map_err(|e| {
                warn!(
                    %domain,
                    error = %e,
                    "Sync command failed"
                );
                e
            })
    }
}

/// Map discovery errors to gRPC status.
pub fn map_discovery_error(e: DiscoveryError) -> Status {
    match e {
        DiscoveryError::DomainNotFound(d) => {
            warn!(domain = %d, "No service registered for domain");
            Status::not_found(format!("No service registered for domain: {}", d))
        }
        DiscoveryError::NoServicesFound(c) => {
            warn!(component = %c, "No services found for component");
            Status::not_found(format!("No services found for component: {}", c))
        }
        DiscoveryError::ConnectionFailed {
            service,
            address,
            message,
        } => {
            warn!(
                service = %service,
                address = %address,
                error = %message,
                "Service connection failed"
            );
            Status::unavailable(format!(
                "Service {} at {} unavailable: {}",
                service, address, message
            ))
        }
        DiscoveryError::KubeError(e) => {
            warn!(error = %e, "Kubernetes API error");
            Status::internal(format!("Kubernetes API error: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::Cover;

    fn make_test_command(domain: &str) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(crate::proto::Uuid {
                    value: uuid::Uuid::new_v4().as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: None,
            }),
            pages: vec![],
            saga_origin: None,
        }
    }

    #[tokio::test]
    async fn test_ensure_correlation_id_empty_stays_empty() {
        let mut command = make_test_command("orders");
        assert!(command.cover.as_ref().unwrap().correlation_id.is_empty());

        let correlation_id = CommandRouter::ensure_correlation_id(&mut command).unwrap();

        assert!(correlation_id.is_empty());
    }

    #[tokio::test]
    async fn test_ensure_correlation_id_preserves_existing() {
        let mut command = make_test_command("orders");
        if let Some(ref mut cover) = command.cover {
            cover.correlation_id = "my-custom-id".to_string();
        }

        let correlation_id = CommandRouter::ensure_correlation_id(&mut command).unwrap();

        assert_eq!(correlation_id, "my-custom-id");
        assert_eq!(
            command.cover.as_ref().unwrap().correlation_id,
            "my-custom-id"
        );
    }
}
