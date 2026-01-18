//! Aggregate service (AggregateCoordinator).

use std::sync::Arc;

use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::{Request, Response, Status};
use tracing::warn;

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::storage::{EventStore, SnapshotStore};
use crate::proto::{
    aggregate_client::AggregateClient, aggregate_coordinator_server::AggregateCoordinator,
    CommandBook, CommandResponse, EventBook, Projection, SyncCommandBook, SyncEventBook,
};
use crate::repository::EventBookRepository;

use crate::utils::response_builder::{
    extract_events_from_response, generate_correlation_id, publish_and_build_response,
};
use crate::utils::sequence_validator::{
    handle_storage_error, validate_sequence, SequenceValidationResult, StorageErrorOutcome,
};
use super::snapshot_handler::persist_snapshot_if_present;

/// Aggregate service.
///
/// Receives commands, loads prior state, calls business logic,
/// persists new events, and notifies projectors/sagas.
pub struct AggregateService {
    event_store: Arc<dyn EventStore>,
    event_book_repo: Arc<EventBookRepository>,
    snapshot_store: Arc<dyn SnapshotStore>,
    business_client: Arc<Mutex<AggregateClient<Channel>>>,
    event_bus: Arc<dyn EventBus>,
    /// When false, snapshots are not written even if business logic returns snapshot_state.
    snapshot_write_enabled: bool,
    /// Service discovery for projectors and sagas (sync operations).
    /// Uses K8s labels to discover services, mesh handles L7 load balancing.
    discovery: Option<Arc<ServiceDiscovery>>,
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
        }
    }

    /// Set the service discovery for sync operations.
    ///
    /// Uses K8s labels to discover projector and saga coordinators.
    /// Service mesh handles L7 gRPC load balancing.
    pub fn with_discovery(mut self, discovery: Arc<ServiceDiscovery>) -> Self {
        self.discovery = Some(discovery);
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

    /// Call saga coordinators synchronously (for CASCADE mode).
    ///
    /// Discovers saga coordinators that listen to this event's source domain.
    /// Uses K8s labels (angzarr.io/source-domain) to filter sagas.
    async fn call_sagas_sync(
        &self,
        event_book: &EventBook,
        sync_mode: crate::proto::SyncMode,
    ) -> Result<(), Status> {
        let discovery = match &self.discovery {
            Some(d) => d,
            None => return Ok(()),
        };

        // Get the source domain from the event's cover
        let source_domain = event_book
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("*");

        // Only get sagas that listen to this domain
        let clients = discovery.get_sagas_for_source(source_domain).await.map_err(|e| {
            warn!(error = %e, "Failed to get saga coordinator clients");
            Status::unavailable(format!("Saga discovery failed: {e}"))
        })?;

        for mut client in clients {
            let request = Request::new(SyncEventBook {
                events: Some(event_book.clone()),
                sync_mode: sync_mode.into(),
            });
            match client.handle_sync(request).await {
                Ok(_) => {}
                Err(e) if e.code() == tonic::Code::NotFound => {
                    // Saga doesn't handle this event type - skip
                }
                Err(e) => {
                    warn!(error = %e, "Saga sync call failed");
                    return Err(Status::internal(format!("Saga sync failed: {e}")));
                }
            }
        }

        Ok(())
    }
}

#[tonic::async_trait]
impl AggregateCoordinator for AggregateService {
    async fn handle(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let command_book = request.into_inner();
        let auto_resequence = command_book.auto_resequence;

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

        // Retry loop for auto_resequence
        let mut attempt = 0;
        loop {
            attempt += 1;

            // Validate CommandBook has pages
            let first_page = command_book.pages.first().ok_or_else(|| {
                Status::invalid_argument("CommandBook must have at least one page")
            })?;

            // 1. Quick sequence check (avoids loading full events if stale)
            if !auto_resequence {
                let expected_sequence = first_page.sequence;

                // Query current aggregate sequence (lightweight operation)
                let next_sequence = self
                    .event_store
                    .get_next_sequence(&domain, root_uuid)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to get sequence: {e}")))?;

                // Validate sequence before loading full events
                match validate_sequence(expected_sequence, next_sequence, auto_resequence) {
                    SequenceValidationResult::Valid => {}
                    SequenceValidationResult::Mismatch { expected, actual } => {
                        return Err(crate::utils::sequence_validator::sequence_mismatch_error(
                            expected, actual,
                        ));
                    }
                }
            }

            // 2. Load prior state (only after sequence validation passes)
            let prior_events = self
                .event_book_repo
                .get(&domain, root_uuid)
                .await
                .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;

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

            // Persist new events - may fail with sequence conflict
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

                    return publish_and_build_response(&self.event_bus, new_events).await;
                }
                Err(e) => {
                    match handle_storage_error(e, &domain, root_uuid, attempt, auto_resequence) {
                        StorageErrorOutcome::Retry => continue,
                        StorageErrorOutcome::Abort(status) => return Err(status),
                    }
                }
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
        let auto_resequence = command_book.auto_resequence;

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

        let mut attempt = 0;
        loop {
            attempt += 1;

            let first_page = command_book.pages.first().ok_or_else(|| {
                Status::invalid_argument("CommandBook must have at least one page")
            })?;

            if !auto_resequence {
                let expected_sequence = first_page.sequence;
                let next_sequence = self
                    .event_store
                    .get_next_sequence(&domain, root_uuid)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to get sequence: {e}")))?;

                match validate_sequence(expected_sequence, next_sequence, auto_resequence) {
                    SequenceValidationResult::Valid => {}
                    SequenceValidationResult::Mismatch { expected, actual } => {
                        return Err(crate::utils::sequence_validator::sequence_mismatch_error(
                            expected, actual,
                        ));
                    }
                }
            }

            let prior_events = self
                .event_book_repo
                .get(&domain, root_uuid)
                .await
                .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;

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

                    // Call sagas synchronously (for CASCADE mode)
                    self.call_sagas_sync(&new_events, sync_mode).await?;

                    return Ok(Response::new(CommandResponse {
                        events: Some(new_events),
                        projections,
                    }));
                }
                Err(e) => {
                    match handle_storage_error(e, &domain, root_uuid, attempt, auto_resequence) {
                        StorageErrorOutcome::Retry => continue,
                        StorageErrorOutcome::Abort(status) => return Err(status),
                    }
                }
            }
        }
    }
}
