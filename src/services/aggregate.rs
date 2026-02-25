//! Aggregate service (AggregateCoordinator).

use std::sync::Arc;

use tonic::transport::Channel;
use tonic::{Request, Response, Status};

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::orchestration::aggregate::grpc::GrpcAggregateContext;
use crate::orchestration::aggregate::{
    execute_command_pipeline, execute_command_with_retry, parse_command_cover, AggregateContext,
    ClientLogic, GrpcBusinessLogic, PipelineMode, TemporalQuery,
};
use crate::proto::{
    aggregate_coordinator_service_server::AggregateCoordinatorService,
    aggregate_service_client::AggregateServiceClient, business_response, BusinessResponse,
    CommandBook, CommandResponse, ContextualCommand, SpeculateAggregateRequest, SyncCommandBook,
};
use crate::proto_ext::CoverExt;
use crate::services::upcaster::Upcaster;
use crate::storage::{EventStore, SnapshotStore};
use crate::utils::retry::saga_backoff;

/// Aggregate service.
///
/// Receives commands, loads prior state, calls client logic,
/// persists new events, and notifies projectors.
///
/// Uses the shared aggregate pipeline for both async and sync operations.
pub struct AggregateService {
    event_store: Arc<dyn EventStore>,
    snapshot_store: Arc<dyn SnapshotStore>,
    business: Arc<dyn ClientLogic>,
    event_bus: Arc<dyn EventBus>,
    /// When false, snapshots are not written even if client logic returns snapshot_state.
    snapshot_write_enabled: bool,
    /// When false, snapshots are not read (for testing/debugging).
    snapshot_read_enabled: bool,
    /// Service discovery for projectors (sync operations).
    discovery: Arc<dyn ServiceDiscovery>,
    /// Upcaster for event version transformation.
    upcaster: Option<Arc<Upcaster>>,
}

impl AggregateService {
    /// Create a new aggregate service with snapshots enabled.
    pub fn new(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        business_client: AggregateServiceClient<Channel>,
        event_bus: Arc<dyn EventBus>,
        discovery: Arc<dyn ServiceDiscovery>,
    ) -> Self {
        Self {
            event_store,
            snapshot_store,
            business: Arc::new(GrpcBusinessLogic::new(business_client)),
            event_bus,
            snapshot_write_enabled: true,
            snapshot_read_enabled: true,
            discovery,
            upcaster: None,
        }
    }

    /// Create a new aggregate service with configurable snapshot behavior.
    pub fn with_config(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        business_client: AggregateServiceClient<Channel>,
        event_bus: Arc<dyn EventBus>,
        discovery: Arc<dyn ServiceDiscovery>,
        snapshot_read_enabled: bool,
        snapshot_write_enabled: bool,
    ) -> Self {
        Self {
            event_store,
            snapshot_store,
            business: Arc::new(GrpcBusinessLogic::new(business_client)),
            event_bus,
            snapshot_write_enabled,
            snapshot_read_enabled,
            discovery,
            upcaster: None,
        }
    }

    /// Set the upcaster for event version transformation.
    pub fn with_upcaster(mut self, upcaster: Arc<Upcaster>) -> Self {
        self.upcaster = Some(upcaster);
        self
    }

    /// Create an async context (no sync projector calls).
    fn create_async_context(&self) -> GrpcAggregateContext {
        let mut ctx = GrpcAggregateContext::with_config(
            self.event_store.clone(),
            self.snapshot_store.clone(),
            self.discovery.clone(),
            self.event_bus.clone(),
            self.snapshot_read_enabled,
            self.snapshot_write_enabled,
        );
        if let Some(ref upcaster) = self.upcaster {
            ctx = ctx.with_upcaster(upcaster.clone());
        }
        ctx
    }

    /// Create a sync context (calls sync projectors).
    fn create_sync_context(&self, sync_mode: crate::proto::SyncMode) -> GrpcAggregateContext {
        let mut ctx = GrpcAggregateContext::with_config(
            self.event_store.clone(),
            self.snapshot_store.clone(),
            self.discovery.clone(),
            self.event_bus.clone(),
            self.snapshot_read_enabled,
            self.snapshot_write_enabled,
        )
        .with_sync_mode(sync_mode);
        if let Some(ref upcaster) = self.upcaster {
            ctx = ctx.with_upcaster(upcaster.clone());
        }
        ctx
    }
}

#[tonic::async_trait]
impl AggregateCoordinatorService for AggregateService {
    /// Handle command asynchronously - publishes to bus, doesn't wait for projectors.
    #[tracing::instrument(name = "aggregate.handle", skip_all)]
    async fn handle(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let command_book = request.into_inner();
        let ctx = self.create_async_context();

        let result =
            execute_command_with_retry(&ctx, &*self.business, command_book, saga_backoff()).await;

        Ok(Response::new(result?))
    }

    /// Handle command synchronously - waits for projectors to complete.
    #[tracing::instrument(name = "aggregate.handle_sync", skip_all)]
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

        let ctx = self.create_sync_context(sync_mode);

        let result =
            execute_command_with_retry(&ctx, &*self.business, command_book, saga_backoff()).await;

        Ok(Response::new(result?))
    }

    /// Speculative: execute command against temporal state without persisting.
    #[tracing::instrument(name = "aggregate.handle_sync_speculative", skip_all)]
    async fn handle_sync_speculative(
        &self,
        request: Request<SpeculateAggregateRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let speculate_req = request.into_inner();
        let command_book = speculate_req.command.ok_or_else(|| {
            Status::invalid_argument("SpeculateAggregateRequest must have a command")
        })?;

        let (as_of_sequence, as_of_timestamp) = match speculate_req.point_in_time {
            Some(temporal) => match temporal.point_in_time {
                Some(crate::proto::temporal_query::PointInTime::AsOfSequence(seq)) => {
                    (Some(seq), None)
                }
                Some(crate::proto::temporal_query::PointInTime::AsOfTime(ts)) => {
                    let ts_str = format!("{}.{}", ts.seconds, ts.nanos);
                    (None, Some(ts_str))
                }
                None => (None, None),
            },
            None => (None, None),
        };

        let ctx = self.create_async_context();

        let response = execute_command_pipeline(
            &ctx,
            &*self.business,
            command_book,
            PipelineMode::Speculative {
                as_of_sequence,
                as_of_timestamp,
            },
        )
        .await?;

        Ok(Response::new(response))
    }

    /// Handle compensation flow - returns BusinessResponse for saga compensation handling.
    ///
    /// Unlike normal Handle, this returns the raw BusinessResponse so the caller
    /// can inspect revocation flags and decide how to handle (quarantine, notify, etc.).
    /// If business logic returns events, they are persisted before returning.
    #[tracing::instrument(name = "aggregate.handle_compensation", skip_all)]
    async fn handle_compensation(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<BusinessResponse>, Status> {
        let command_book = request.into_inner();
        let (domain, root_uuid) = parse_command_cover(&command_book)?;
        let edition = command_book.edition().to_string();
        let correlation_id =
            crate::orchestration::correlation::extract_correlation_id(&command_book)?;

        let ctx = self.create_async_context();

        // Load prior events
        let prior_events = ctx
            .load_prior_events(&domain, &edition, root_uuid, &TemporalQuery::Current)
            .await?;

        // Transform events (upcasting)
        let prior_events = ctx.transform_events(&domain, prior_events).await?;

        // Invoke business logic
        let contextual_command = ContextualCommand {
            events: Some(prior_events.clone()),
            command: Some(command_book),
        };

        let response = self.business.invoke(contextual_command).await?;

        // If business returned events, persist them
        if let Some(business_response::Result::Events(ref events)) = response.result {
            if !events.pages.is_empty() {
                ctx.persist_events(
                    &prior_events,
                    events,
                    &domain,
                    &edition,
                    root_uuid,
                    &correlation_id,
                )
                .await?;

                // Post-persist: publish to bus
                ctx.post_persist(events).await?;
            }
        }

        Ok(Response::new(response))
    }
}
