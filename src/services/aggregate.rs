//! Aggregate service (AggregateCoordinator).

use std::sync::Arc;

use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::{Request, Response, Status};

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::orchestration::aggregate::grpc::GrpcAggregateContext;
use crate::orchestration::aggregate::{execute_command_pipeline, PipelineMode};
use crate::proto::{
    aggregate_client::AggregateClient, aggregate_coordinator_server::AggregateCoordinator,
    CommandBook, CommandResponse, DryRunRequest, SyncCommandBook,
};
use crate::services::upcaster::Upcaster;
use crate::storage::{EventStore, SnapshotStore};

/// Aggregate service.
///
/// Receives commands, loads prior state, calls business logic via gRPC,
/// persists new events, and notifies projectors.
///
/// Uses the shared aggregate pipeline for both async and sync operations.
pub struct AggregateService {
    event_store: Arc<dyn EventStore>,
    snapshot_store: Arc<dyn SnapshotStore>,
    business_client: Arc<Mutex<AggregateClient<Channel>>>,
    event_bus: Arc<dyn EventBus>,
    /// When false, snapshots are not written even if business logic returns snapshot_state.
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
        business_client: AggregateClient<Channel>,
        event_bus: Arc<dyn EventBus>,
        discovery: Arc<dyn ServiceDiscovery>,
    ) -> Self {
        Self {
            event_store,
            snapshot_store,
            business_client: Arc::new(Mutex::new(business_client)),
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
        business_client: AggregateClient<Channel>,
        event_bus: Arc<dyn EventBus>,
        discovery: Arc<dyn ServiceDiscovery>,
        snapshot_read_enabled: bool,
        snapshot_write_enabled: bool,
    ) -> Self {
        Self {
            event_store,
            snapshot_store,
            business_client: Arc::new(Mutex::new(business_client)),
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
impl AggregateCoordinator for AggregateService {
    /// Handle command asynchronously - publishes to bus, doesn't wait for projectors.
    async fn handle(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let command_book = request.into_inner();
        let ctx = self.create_async_context();

        let response = execute_command_pipeline(
            &ctx,
            &self.business_client,
            command_book,
            PipelineMode::Execute {
                validate_sequence: true,
            },
        )
        .await?;

        Ok(Response::new(response))
    }

    /// Handle command synchronously - waits for projectors to complete.
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

        let response = execute_command_pipeline(
            &ctx,
            &self.business_client,
            command_book,
            PipelineMode::Execute {
                validate_sequence: true,
            },
        )
        .await?;

        Ok(Response::new(response))
    }

    /// Dry-run: execute command against temporal state without persisting.
    async fn dry_run_handle(
        &self,
        request: Request<DryRunRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let dry_run = request.into_inner();
        let command_book = dry_run
            .command
            .ok_or_else(|| Status::invalid_argument("DryRunRequest must have a command"))?;

        let (as_of_sequence, as_of_timestamp) = match dry_run.point_in_time {
            Some(temporal) => match temporal.point_in_time {
                Some(crate::proto::temporal_query::PointInTime::AsOfSequence(seq)) => {
                    (Some(seq), None)
                }
                Some(crate::proto::temporal_query::PointInTime::AsOfTime(ts)) => {
                    let ts_str = format!(
                        "{}.{}",
                        ts.seconds,
                        ts.nanos
                    );
                    (None, Some(ts_str))
                }
                None => (None, None),
            },
            None => (None, None),
        };

        let ctx = self.create_async_context();

        let response = execute_command_pipeline(
            &ctx,
            &self.business_client,
            command_book,
            PipelineMode::DryRun {
                as_of_sequence,
                as_of_timestamp,
            },
        )
        .await?;

        Ok(Response::new(response))
    }
}
