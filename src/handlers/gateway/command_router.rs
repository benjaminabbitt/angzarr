//! Command routing for gateway service.
//!
//! Handles forwarding commands to aggregate services based on domain routing,
//! using K8s label-based service discovery.

use std::sync::Arc;

use prost::Message;
use tonic::{Request, Status};
use tracing::{debug, warn};

use crate::discovery::{DiscoveryError, ServiceDiscovery};
use crate::proto::{CommandBook, CommandResponse, SyncCommandBook};

/// Command router for forwarding commands to aggregate services.
#[derive(Clone)]
pub struct CommandRouter {
    discovery: Arc<ServiceDiscovery>,
}

impl CommandRouter {
    /// Create a new command router with service discovery.
    pub fn new(discovery: Arc<ServiceDiscovery>) -> Self {
        Self { discovery }
    }

    /// Generate or use existing correlation ID.
    #[allow(clippy::result_large_err)]
    pub fn ensure_correlation_id(command_book: &mut CommandBook) -> Result<String, Status> {
        let current_correlation_id = command_book
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        if current_correlation_id.is_empty() {
            let mut buf = Vec::new();
            command_book
                .encode(&mut buf)
                .map_err(|e| Status::internal(format!("Failed to encode command: {e}")))?;
            let angzarr_ns = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"angzarr.dev");
            let generated = uuid::Uuid::new_v5(&angzarr_ns, &buf).to_string();
            if let Some(ref mut cover) = command_book.cover {
                cover.correlation_id = generated.clone();
            }
            Ok(generated)
        } else {
            Ok(current_correlation_id)
        }
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
            .unwrap_or_else(|| "*".to_string());

        debug!(
            correlation_id = %correlation_id,
            domain = %domain,
            "Routing command to domain"
        );

        let mut client = self
            .discovery
            .get_aggregate(&domain)
            .await
            .map_err(map_discovery_error)?;

        client
            .handle(Request::new(command_book))
            .await
            .map(|r| r.into_inner())
            .map_err(|e| {
                warn!(
                    correlation_id = %correlation_id,
                    domain = %domain,
                    error = %e,
                    "Command failed"
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
            .unwrap_or_else(|| "*".to_string());

        debug!(
            correlation_id = %correlation_id,
            domain = %domain,
            sync_mode = %sync_mode,
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
            .handle_sync(Request::new(sync_request))
            .await
            .map(|r| r.into_inner())
            .map_err(|e| {
                warn!(
                    correlation_id = %correlation_id,
                    domain = %domain,
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
            }),
            pages: vec![],
            saga_origin: None,
        }
    }

    #[tokio::test]
    async fn test_ensure_correlation_id_generates_when_empty() {
        let mut command = make_test_command("orders");
        assert!(command.cover.as_ref().unwrap().correlation_id.is_empty());

        let correlation_id = CommandRouter::ensure_correlation_id(&mut command).unwrap();

        assert!(!correlation_id.is_empty());
        assert_eq!(
            command.cover.as_ref().unwrap().correlation_id,
            correlation_id
        );
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
