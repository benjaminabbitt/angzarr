//! Command routing for gateway service.
//!
//! Handles forwarding commands to aggregate services based on domain routing,
//! using K8s label-based service discovery.

#![allow(clippy::result_large_err)]

use std::sync::Arc;

use tonic::Status;
use tracing::{debug, warn};

use crate::discovery::{DiscoveryError, ServiceDiscovery};
use crate::proto::{CommandBook, CommandResponse, DryRunRequest, SyncCommandBook};
use crate::proto_ext::{correlated_request, WILDCARD_DOMAIN};
use crate::validation;

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

    /// Extract and validate domain from command cover.
    fn extract_domain(command_book: &CommandBook) -> Result<String, Status> {
        let domain = command_book
            .cover
            .as_ref()
            .map(|c| c.domain.clone())
            .unwrap_or_else(|| WILDCARD_DOMAIN.to_string());
        validation::validate_domain(&domain)?;
        Ok(domain)
    }

    /// Forward command to aggregate coordinator based on domain.
    pub async fn forward_command(
        &self,
        command_book: CommandBook,
        correlation_id: &str,
    ) -> Result<CommandResponse, Status> {
        let domain = Self::extract_domain(&command_book)?;

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
        validation::validate_domain(&domain)?;

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
        let domain = Self::extract_domain(&command_book)?;

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
///
/// Error messages are sanitized to avoid leaking infrastructure details.
/// Full details are logged internally at DEBUG level.
pub fn map_discovery_error(e: DiscoveryError) -> Status {
    use super::errmsg;

    match e {
        DiscoveryError::DomainNotFound(d) => {
            // Log full details internally
            debug!(domain = %d, "No service registered for domain");
            // Return sanitized message to client
            Status::not_found(errmsg::DOMAIN_NOT_FOUND)
        }
        DiscoveryError::NoServicesFound(c) => {
            debug!(component = %c, "No services found for component");
            Status::not_found(errmsg::COMPONENT_NOT_FOUND)
        }
        DiscoveryError::ConnectionFailed {
            service,
            address,
            message,
        } => {
            // Log full details (including address) internally
            debug!(
                service = %service,
                address = %address,
                error = %message,
                "Service connection failed"
            );
            // Return sanitized message without infrastructure details
            Status::unavailable(errmsg::SERVICE_UNAVAILABLE)
        }
        DiscoveryError::KubeError(e) => {
            // Log K8s error details internally
            debug!(error = %e, "Kubernetes API error");
            // Return generic message to avoid exposing infrastructure
            Status::internal(errmsg::INTERNAL_ERROR)
        }
    }
}
