//! Aggregate service (AggregateCoordinator).

use std::sync::Arc;

use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::{Request, Response, Status};
use tracing::warn;

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::proto::{
    aggregate_client::AggregateClient, aggregate_coordinator_server::AggregateCoordinator,
    CommandBook, CommandResponse, EventBook, Projection, SyncCommandBook, SyncEventBook,
};
use crate::repository::EventBookRepository;
use crate::services::upcaster::Upcaster;
use crate::storage::{EventStore, SnapshotStore};

use super::snapshot_handler::persist_snapshot_if_present;
use crate::utils::response_builder::{
    extract_events_from_response, generate_correlation_id, publish_and_build_response,
};
use crate::utils::sequence_validator::{
    handle_storage_error, sequence_mismatch_error_with_state, validate_sequence,
    SequenceValidationResult, StorageErrorOutcome,
};

/// Aggregate service.
///
/// Receives commands, loads prior state, calls business logic,
/// persists new events, and notifies projectors.
pub struct AggregateService {
    event_store: Arc<dyn EventStore>,
    event_book_repo: Arc<EventBookRepository>,
    snapshot_store: Arc<dyn SnapshotStore>,
    business_client: Arc<Mutex<AggregateClient<Channel>>>,
    event_bus: Arc<dyn EventBus>,
    /// When false, snapshots are not written even if business logic returns snapshot_state.
    snapshot_write_enabled: bool,
    /// Service discovery for projectors (sync operations).
    /// Uses K8s labels to discover services, mesh handles L7 load balancing.
    discovery: Option<Arc<ServiceDiscovery>>,
    /// Upcaster for event version transformation.
    upcaster: Option<Arc<Upcaster>>,
}

impl AggregateService {
    /// Create a new aggregate service with snapshots enabled.
    pub fn new(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        business_client: AggregateClient<Channel>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            event_store: Arc::clone(&event_store),
            event_book_repo: Arc::new(EventBookRepository::new(
                event_store,
                Arc::clone(&snapshot_store),
            )),
            snapshot_store,
            business_client: Arc::new(Mutex::new(business_client)),
            event_bus,
            snapshot_write_enabled: true,
            discovery: None,
            upcaster: None,
        }
    }

    /// Create a new aggregate service with configurable snapshot behavior.
    pub fn with_config(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        business_client: AggregateClient<Channel>,
        event_bus: Arc<dyn EventBus>,
        snapshot_read_enabled: bool,
        snapshot_write_enabled: bool,
    ) -> Self {
        Self {
            event_store: Arc::clone(&event_store),
            event_book_repo: Arc::new(EventBookRepository::with_config(
                event_store,
                Arc::clone(&snapshot_store),
                snapshot_read_enabled,
            )),
            snapshot_store,
            business_client: Arc::new(Mutex::new(business_client)),
            event_bus,
            snapshot_write_enabled,
            discovery: None,
            upcaster: None,
        }
    }

    /// Set the service discovery for sync operations.
    ///
    /// Uses K8s labels to discover projector coordinators.
    /// Service mesh handles L7 gRPC load balancing.
    pub fn with_discovery(mut self, discovery: Arc<ServiceDiscovery>) -> Self {
        self.discovery = Some(discovery);
        self
    }

    /// Set the upcaster for event version transformation.
    ///
    /// When set, events loaded from storage are passed through the upcaster
    /// before being sent to business logic.
    pub fn with_upcaster(mut self, upcaster: Arc<Upcaster>) -> Self {
        self.upcaster = Some(upcaster);
        self
    }

    /// Call projector coordinators synchronously and return projections.
    ///
    /// Discovers all projector coordinators via K8s labels.
    /// Mesh handles pod-level load balancing.
    async fn call_projectors_sync(
        &self,
        event_book: &EventBook,
        sync_mode: crate::proto::SyncMode,
    ) -> Result<Vec<Projection>, Status> {
        let discovery = match &self.discovery {
            Some(d) => d,
            None => return Ok(vec![]),
        };

        let clients = discovery.get_all_projectors().await.map_err(|e| {
            warn!(error = %e, "Failed to get projector coordinator clients");
            Status::unavailable(format!("Projector discovery failed: {e}"))
        })?;

        if clients.is_empty() {
            return Ok(vec![]);
        }

        let mut projections = Vec::new();
        for mut client in clients {
            let request = Request::new(SyncEventBook {
                events: Some(event_book.clone()),
                sync_mode: sync_mode.into(),
            });
            match client.handle_sync(request).await {
                Ok(response) => projections.push(response.into_inner()),
                Err(e) if e.code() == tonic::Code::NotFound => {
                    // Projector doesn't handle this domain - skip
                }
                Err(e) => {
                    warn!(error = %e, "Projector sync call failed");
                    return Err(Status::internal(format!("Projector sync failed: {e}")));
                }
            }
        }

        Ok(projections)
    }
}

#[tonic::async_trait]
impl AggregateCoordinator for AggregateService {
    async fn handle(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let command_book = request.into_inner();

        // Extract cover (aggregate identity)
        let cover = command_book
            .cover
            .clone()
            .ok_or_else(|| Status::invalid_argument("CommandBook must have a cover"))?;

        let domain = cover.domain.clone();
        let root = cover
            .root
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Cover must have a root UUID"))?;

        let root_uuid = uuid::Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

        // Generate correlation ID if not provided
        let correlation_id = generate_correlation_id(&command_book)?;

        // Validate CommandBook has pages
        let first_page = command_book.pages.first().ok_or_else(|| {
            Status::invalid_argument("CommandBook must have at least one page")
        })?;

        // Get expected sequence from command
        let expected_sequence = first_page.sequence;

        // Query current aggregate sequence (lightweight operation)
        let next_sequence = self
            .event_store
            .get_next_sequence(&domain, root_uuid)
            .await
            .map_err(|e| Status::internal(format!("Failed to get sequence: {e}")))?;

        // Validate sequence - on mismatch, load EventBook and return with error
        match validate_sequence(expected_sequence, next_sequence) {
            SequenceValidationResult::Valid => {}
            SequenceValidationResult::Mismatch { expected, actual } => {
                // Load EventBook for error details so caller can retry without extra fetch
                let prior_events = self
                    .event_book_repo
                    .get(&domain, root_uuid)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;
                return Err(sequence_mismatch_error_with_state(
                    expected,
                    actual,
                    &prior_events,
                ));
            }
        }

        // Load prior state (only after sequence validation passes)
        let mut prior_events = self
            .event_book_repo
            .get(&domain, root_uuid)
            .await
            .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;

        // Upcast events if upcaster is configured
        if let Some(ref upcaster) = self.upcaster {
            let upcasted_pages = upcaster
                .upcast(&domain, prior_events.pages)
                .await
                .map_err(|e| Status::internal(format!("Upcaster failed: {e}")))?;
            prior_events.pages = upcasted_pages;
        }

        // Create contextual command
        let contextual_command = crate::proto::ContextualCommand {
            events: Some(prior_events),
            command: Some(command_book.clone()),
        };

        // Call business logic
        let response = self
            .business_client
            .lock()
            .await
            .handle(contextual_command)
            .await?
            .into_inner();

        // Extract events from BusinessResponse
        let new_events = extract_events_from_response(response, correlation_id.clone())?;

        // Persist new events
        match self.event_book_repo.put(&new_events).await {
            Ok(()) => {
                // Success - store snapshot and publish
                persist_snapshot_if_present(
                    &self.snapshot_store,
                    &new_events,
                    &domain,
                    root_uuid,
                    self.snapshot_write_enabled,
                )
                .await?;

                publish_and_build_response(&self.event_bus, new_events).await
            }
            Err(e) => {
                let StorageErrorOutcome::Abort(status) = handle_storage_error(e, &domain, root_uuid);
                Err(status)
            }
        }
    }

    /// Handle command synchronously - waits for projectors to complete.
    ///
    /// Same as handle() but calls projector_coordinator.handle_sync instead of
    /// async event bus publish. Returns sync projector results in response.
    async fn handle_sync(
        &self,
        request: Request<SyncCommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let sync_request = request.into_inner();
        let sync_mode = crate::proto::SyncMode::try_from(sync_request.sync_mode)
            .unwrap_or(crate::proto::SyncMode::Simple);
        let command_book = sync_request
            .command
            .ok_or_else(|| Status::invalid_argument("SyncCommandBook must have a command"))?;

        let cover = command_book
            .cover
            .clone()
            .ok_or_else(|| Status::invalid_argument("CommandBook must have a cover"))?;

        let domain = cover.domain.clone();
        let root = cover
            .root
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Cover must have a root UUID"))?;

        let root_uuid = uuid::Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

        let correlation_id = generate_correlation_id(&command_book)?;

        let first_page = command_book.pages.first().ok_or_else(|| {
            Status::invalid_argument("CommandBook must have at least one page")
        })?;

        let expected_sequence = first_page.sequence;

        // Query current aggregate sequence
        let next_sequence = self
            .event_store
            .get_next_sequence(&domain, root_uuid)
            .await
            .map_err(|e| Status::internal(format!("Failed to get sequence: {e}")))?;

        // Validate sequence - on mismatch, load EventBook and return with error
        match validate_sequence(expected_sequence, next_sequence) {
            SequenceValidationResult::Valid => {}
            SequenceValidationResult::Mismatch { expected, actual } => {
                let prior_events = self
                    .event_book_repo
                    .get(&domain, root_uuid)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;
                return Err(sequence_mismatch_error_with_state(
                    expected,
                    actual,
                    &prior_events,
                ));
            }
        }

        let mut prior_events = self
            .event_book_repo
            .get(&domain, root_uuid)
            .await
            .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;

        // Upcast events if upcaster is configured
        if let Some(ref upcaster) = self.upcaster {
            let upcasted_pages = upcaster
                .upcast(&domain, prior_events.pages)
                .await
                .map_err(|e| Status::internal(format!("Upcaster failed: {e}")))?;
            prior_events.pages = upcasted_pages;
        }

        let contextual_command = crate::proto::ContextualCommand {
            events: Some(prior_events),
            command: Some(command_book.clone()),
        };

        let response = self
            .business_client
            .lock()
            .await
            .handle(contextual_command)
            .await?
            .into_inner();

        let new_events = extract_events_from_response(response, correlation_id.clone())?;

        match self.event_book_repo.put(&new_events).await {
            Ok(()) => {
                persist_snapshot_if_present(
                    &self.snapshot_store,
                    &new_events,
                    &domain,
                    root_uuid,
                    self.snapshot_write_enabled,
                )
                .await?;

                // Call projectors synchronously
                let projections = self.call_projectors_sync(&new_events, sync_mode).await?;

                Ok(Response::new(CommandResponse {
                    events: Some(new_events),
                    projections,
                }))
            }
            Err(e) => {
                let StorageErrorOutcome::Abort(status) = handle_storage_error(e, &domain, root_uuid);
                Err(status)
            }
        }
    }
}
