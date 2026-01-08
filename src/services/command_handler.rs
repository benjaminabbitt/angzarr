//! Command handler service (BusinessCoordinator).

use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::interfaces::{BusinessError, BusinessLogicClient, EventBus, EventStore, SnapshotStore};
use crate::proto::{
    business_coordinator_server::BusinessCoordinator, CommandBook, EventBook,
    SynchronousProcessingResponse,
};
use crate::repository::EventBookRepository;

/// Command handler service.
///
/// Receives commands, loads prior state, calls business logic,
/// persists new events, and notifies projectors/sagas.
pub struct CommandHandlerService {
    event_book_repo: Arc<EventBookRepository>,
    business_client: Arc<dyn BusinessLogicClient>,
    event_bus: Arc<dyn EventBus>,
}

impl CommandHandlerService {
    /// Create a new command handler service.
    pub fn new(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        business_client: Arc<dyn BusinessLogicClient>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            event_book_repo: Arc::new(EventBookRepository::new(event_store, snapshot_store)),
            business_client,
            event_bus,
        }
    }
}

#[tonic::async_trait]
impl BusinessCoordinator for CommandHandlerService {
    async fn handle(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<SynchronousProcessingResponse>, Status> {
        let command_book = request.into_inner();

        // Extract cover (aggregate identity)
        let cover = command_book
            .cover
            .clone()
            .ok_or_else(|| Status::invalid_argument("CommandBook must have a cover"))?;

        let domain = &cover.domain;
        let root = cover
            .root
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Cover must have a root UUID"))?;

        let root_uuid = uuid::Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

        // Validate domain is supported
        if !self.business_client.has_domain(domain) {
            return Err(Status::not_found(format!(
                "Domain '{}' not registered. Available: {:?}",
                domain,
                self.business_client.domains()
            )));
        }

        // Load prior state
        let prior_events = self
            .event_book_repo
            .get(domain, root_uuid)
            .await
            .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;

        // Create contextual command
        let contextual_command = crate::proto::ContextualCommand {
            events: Some(prior_events),
            command: Some(command_book),
        };

        // Call business logic
        let new_events = self
            .business_client
            .handle(domain, contextual_command)
            .await
            .map_err(|e| match e {
                BusinessError::DomainNotFound(d) => {
                    Status::not_found(format!("Domain not found: {d}"))
                }
                BusinessError::Rejected(msg) => Status::failed_precondition(msg),
                BusinessError::Timeout { domain } => {
                    Status::deadline_exceeded(format!("Timeout waiting for domain: {domain}"))
                }
                BusinessError::Connection { domain, message } => {
                    Status::unavailable(format!("Connection to {domain} failed: {message}"))
                }
                BusinessError::Grpc(status) => *status,
            })?;

        // Persist new events
        self.event_book_repo
            .put(&new_events)
            .await
            .map_err(|e| Status::internal(format!("Failed to persist events: {e}")))?;

        // Wrap in Arc for immutable distribution
        let new_events = Arc::new(new_events);

        // Notify event bus (projectors, sagas)
        let publish_result = self.event_bus
            .publish(Arc::clone(&new_events))
            .await
            .map_err(|e| Status::internal(format!("Failed to publish events: {e}")))?;

        Ok(Response::new(SynchronousProcessingResponse {
            books: vec![Arc::try_unwrap(new_events).unwrap_or_else(|arc| (*arc).clone())],
            commands: vec![],
            projections: publish_result.projections,
        }))
    }

    async fn record(
        &self,
        request: Request<EventBook>,
    ) -> Result<Response<SynchronousProcessingResponse>, Status> {
        let event_book = request.into_inner();

        // Persist events directly (used by sagas)
        self.event_book_repo
            .put(&event_book)
            .await
            .map_err(|e| Status::internal(format!("Failed to persist events: {e}")))?;

        // Wrap in Arc for immutable distribution
        let event_book = Arc::new(event_book);

        // Notify event bus
        let publish_result = self.event_bus
            .publish(Arc::clone(&event_book))
            .await
            .map_err(|e| Status::internal(format!("Failed to publish events: {e}")))?;

        Ok(Response::new(SynchronousProcessingResponse {
            books: vec![Arc::try_unwrap(event_book).unwrap_or_else(|arc| (*arc).clone())],
            commands: vec![],
            projections: publish_result.projections,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{CommandPage, Cover, Uuid as ProtoUuid};
    use crate::test_utils::{MockBusinessLogic, MockEventBus, MockEventStore, MockSnapshotStore};
    use prost_types::Any;

    fn create_test_service_with_mocks(
        event_store: Arc<MockEventStore>,
        snapshot_store: Arc<MockSnapshotStore>,
        business_client: Arc<MockBusinessLogic>,
        event_bus: Arc<MockEventBus>,
    ) -> CommandHandlerService {
        CommandHandlerService::new(event_store, snapshot_store, business_client, event_bus)
    }

    fn create_default_test_service() -> (
        CommandHandlerService,
        Arc<MockEventStore>,
        Arc<MockSnapshotStore>,
        Arc<MockBusinessLogic>,
        Arc<MockEventBus>,
    ) {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());
        let business_client = Arc::new(MockBusinessLogic::new(vec!["orders".to_string()]));
        let event_bus = Arc::new(MockEventBus::new());

        let service = create_test_service_with_mocks(
            event_store.clone(),
            snapshot_store.clone(),
            business_client.clone(),
            event_bus.clone(),
        );

        (service, event_store, snapshot_store, business_client, event_bus)
    }

    fn make_command_book(domain: &str, root_bytes: Vec<u8>) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid { value: root_bytes }),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(Any {
                    type_url: "test.CreateOrder".to_string(),
                    value: vec![],
                }),
                synchronous: false,
            }],
        }
    }

    #[tokio::test]
    async fn test_handle_command_success() {
        let (service, _, _, _, event_bus) = create_default_test_service();
        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.books.len(), 1);
        assert_eq!(event_bus.published_count().await, 1);
    }

    #[tokio::test]
    async fn test_handle_command_missing_cover() {
        let (service, _, _, _, _) = create_default_test_service();
        let command = CommandBook {
            cover: None,
            pages: vec![],
        };

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("cover"));
    }

    #[tokio::test]
    async fn test_handle_command_missing_root() {
        let (service, _, _, _, _) = create_default_test_service();
        let command = CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: None,
            }),
            pages: vec![],
        };

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("root"));
    }

    #[tokio::test]
    async fn test_handle_command_invalid_uuid() {
        let (service, _, _, _, _) = create_default_test_service();
        let command = CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1, 2, 3], // Invalid: must be 16 bytes
                }),
            }),
            pages: vec![],
        };

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("UUID"));
    }

    #[tokio::test]
    async fn test_handle_command_unknown_domain() {
        let (service, _, _, _, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();
        let command = make_command_book("unknown_domain", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_handle_command_business_logic_rejects() {
        let (service, _, _, business_client, _) = create_default_test_service();
        business_client.set_reject_command(true).await;

        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn test_handle_command_business_logic_connection_failure() {
        let (service, _, _, business_client, _) = create_default_test_service();
        business_client.set_fail_on_handle(true).await;

        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unavailable);
    }

    #[tokio::test]
    async fn test_handle_command_event_bus_failure() {
        let (service, _, _, _, event_bus) = create_default_test_service();
        event_bus.set_fail_on_publish(true).await;

        let root = uuid::Uuid::new_v4();
        let command = make_command_book("orders", root.as_bytes().to_vec());

        let response = service.handle(Request::new(command)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
    }

    #[tokio::test]
    async fn test_record_events_success() {
        let (service, _, _, _, event_bus) = create_default_test_service();
        let root = uuid::Uuid::new_v4();

        let event_book = crate::proto::EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![crate::proto::EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(0)),
                event: Some(Any {
                    type_url: "test.OrderCreated".to_string(),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            }],
            snapshot: None,
        };

        let response = service.record(Request::new(event_book)).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.books.len(), 1);
        assert_eq!(event_bus.published_count().await, 1);
    }

    #[tokio::test]
    async fn test_record_events_bus_failure() {
        let (service, _, _, _, event_bus) = create_default_test_service();
        event_bus.set_fail_on_publish(true).await;

        let root = uuid::Uuid::new_v4();
        let event_book = crate::proto::EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![],
            snapshot: None,
        };

        let response = service.record(Request::new(event_book)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
    }
}
