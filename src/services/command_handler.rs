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
        self.event_bus
            .publish(Arc::clone(&new_events))
            .await
            .map_err(|e| Status::internal(format!("Failed to publish events: {e}")))?;

        Ok(Response::new(SynchronousProcessingResponse {
            books: vec![Arc::try_unwrap(new_events).unwrap_or_else(|arc| (*arc).clone())],
            projections: vec![],
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
        self.event_bus
            .publish(Arc::clone(&event_book))
            .await
            .map_err(|e| Status::internal(format!("Failed to publish events: {e}")))?;

        Ok(Response::new(SynchronousProcessingResponse {
            books: vec![Arc::try_unwrap(event_book).unwrap_or_else(|arc| (*arc).clone())],
            projections: vec![],
        }))
    }
}
