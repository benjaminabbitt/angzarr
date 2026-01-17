//! Command routing for gateway service.
//!
//! Handles forwarding commands to entity services based on domain routing,
//! service discovery, and gRPC client management.

use std::sync::Arc;

use prost::Message;
use tonic::{Request, Status};
use tracing::{debug, warn};

use crate::discovery::{RegistryError, ServiceRegistry};
use crate::proto::{CommandBook, CommandResponse};

/// Command router for forwarding commands to entity services.
#[derive(Clone)]
pub struct CommandRouter {
    registry: Arc<ServiceRegistry>,
}

impl CommandRouter {
    /// Create a new command router with service registry for domain routing.
    pub fn new(registry: Arc<ServiceRegistry>) -> Self {
        Self { registry }
    }

    /// Get the service registry.
    pub fn registry(&self) -> &Arc<ServiceRegistry> {
        &self.registry
    }

    /// Generate or use existing correlation ID.
    #[allow(clippy::result_large_err)]
    pub fn ensure_correlation_id(command_book: &mut CommandBook) -> Result<String, Status> {
        if command_book.correlation_id.is_empty() {
            let mut buf = Vec::new();
            command_book
                .encode(&mut buf)
                .map_err(|e| Status::internal(format!("Failed to encode command: {e}")))?;
            let angzarr_ns = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"angzarr.dev");
            let generated = uuid::Uuid::new_v5(&angzarr_ns, &buf).to_string();
            command_book.correlation_id = generated.clone();
            Ok(generated)
        } else {
            Ok(command_book.correlation_id.clone())
        }
    }

    /// Forward command to business coordinator based on domain.
    pub async fn forward_command(
        &self,
        command_book: CommandBook,
        correlation_id: &str,
    ) -> Result<CommandResponse, Status> {
        // Extract domain from command cover (clone to avoid borrow issues)
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

        // Get client for domain from registry
        let mut command_client = self
            .registry
            .get_client(&domain)
            .await
            .map_err(map_registry_error)?;

        command_client
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
}

/// Map registry errors to gRPC status.
pub fn map_registry_error(e: RegistryError) -> Status {
    match e {
        RegistryError::DomainNotFound(d) => {
            warn!(domain = %d, "No service registered for domain");
            Status::not_found(format!("No service registered for domain: {}", d))
        }
        RegistryError::ConnectionFailed {
            domain,
            address,
            message,
        } => {
            warn!(
                domain = %domain,
                address = %address,
                error = %message,
                "Service connection failed"
            );
            Status::unavailable(format!(
                "Service {} at {} unavailable: {}",
                domain, address, message
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::ServiceEndpoint;
    use crate::proto::business_coordinator_server::{
        BusinessCoordinator, BusinessCoordinatorServer,
    };
    use crate::proto::{Cover, EventBook, EventPage, Uuid as ProtoUuid};
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tonic::transport::Server;

    /// Mock BusinessCoordinator that returns configurable responses.
    struct MockBusinessCoordinator {
        response_events: Arc<tokio::sync::RwLock<Option<EventBook>>>,
        call_count: Arc<AtomicU32>,
    }

    impl MockBusinessCoordinator {
        fn new() -> Self {
            Self {
                response_events: Arc::new(tokio::sync::RwLock::new(None)),
                call_count: Arc::new(AtomicU32::new(0)),
            }
        }

        fn get_call_count(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl Clone for MockBusinessCoordinator {
        fn clone(&self) -> Self {
            Self {
                response_events: self.response_events.clone(),
                call_count: self.call_count.clone(),
            }
        }
    }

    #[tonic::async_trait]
    impl BusinessCoordinator for MockBusinessCoordinator {
        async fn handle(
            &self,
            request: Request<CommandBook>,
        ) -> Result<tonic::Response<CommandResponse>, Status> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let cmd = request.into_inner();

            let events = self
                .response_events
                .read()
                .await
                .clone()
                .unwrap_or_else(|| EventBook {
                    cover: cmd.cover.clone(),
                    pages: vec![EventPage {
                        sequence: Some(crate::proto::event_page::Sequence::Num(0)),
                        event: Some(prost_types::Any {
                            type_url: "test.Event".to_string(),
                            value: vec![],
                        }),
                        created_at: None,
                        synchronous: false,
                    }],
                    snapshot: None,
                    correlation_id: cmd.correlation_id.clone(),
                    snapshot_state: None,
                });

            Ok(tonic::Response::new(CommandResponse {
                events: Some(events),
                projections: vec![],
            }))
        }

        async fn record(
            &self,
            _request: Request<EventBook>,
        ) -> Result<tonic::Response<CommandResponse>, Status> {
            Ok(tonic::Response::new(CommandResponse {
                events: None,
                projections: vec![],
            }))
        }
    }

    fn make_test_command(domain: &str) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: uuid::Uuid::new_v4().as_bytes().to_vec(),
                }),
            }),
            pages: vec![],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: false,
            fact: false,
        }
    }

    async fn setup_command_router() -> (
        CommandRouter,
        Arc<MockBusinessCoordinator>,
        tokio::task::JoinHandle<()>,
    ) {
        let mock_coordinator = Arc::new(MockBusinessCoordinator::new());

        let coord_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let coord_listener = tokio::net::TcpListener::bind(coord_addr).await.unwrap();
        let coord_port = coord_listener.local_addr().unwrap().port();

        let coord_clone = mock_coordinator.clone();
        let coord_handle = tokio::spawn(async move {
            Server::builder()
                .add_service(BusinessCoordinatorServer::new(coord_clone.as_ref().clone()))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                    coord_listener,
                ))
                .await
                .ok();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let registry = Arc::new(ServiceRegistry::new());
        registry
            .update_endpoint(ServiceEndpoint {
                domain: "*".to_string(),
                address: "127.0.0.1".to_string(),
                port: coord_port,
            })
            .await;

        let router = CommandRouter::new(registry);

        (router, mock_coordinator, coord_handle)
    }

    #[tokio::test]
    async fn test_forward_command_success() {
        let (router, mock_coord, handle) = setup_command_router().await;

        let command = make_test_command("orders");
        let result = router.forward_command(command, "test-correlation").await;

        assert!(result.is_ok());
        assert_eq!(mock_coord.get_call_count(), 1);

        handle.abort();
    }

    #[tokio::test]
    async fn test_ensure_correlation_id_generates_when_empty() {
        let mut command = make_test_command("orders");
        assert!(command.correlation_id.is_empty());

        let correlation_id = CommandRouter::ensure_correlation_id(&mut command).unwrap();

        assert!(!correlation_id.is_empty());
        assert_eq!(command.correlation_id, correlation_id);
    }

    #[tokio::test]
    async fn test_ensure_correlation_id_preserves_existing() {
        let mut command = make_test_command("orders");
        command.correlation_id = "my-custom-id".to_string();

        let correlation_id = CommandRouter::ensure_correlation_id(&mut command).unwrap();

        assert_eq!(correlation_id, "my-custom-id");
        assert_eq!(command.correlation_id, "my-custom-id");
    }

    #[tokio::test]
    async fn test_forward_command_unknown_domain_returns_not_found() {
        let registry = Arc::new(ServiceRegistry::new());
        let router = CommandRouter::new(registry);

        let command = make_test_command("unknown-domain");
        let result = router.forward_command(command, "test-correlation").await;

        match result {
            Err(status) => assert_eq!(status.code(), tonic::Code::NotFound),
            Ok(_) => panic!("Expected not found error"),
        }
    }
}
