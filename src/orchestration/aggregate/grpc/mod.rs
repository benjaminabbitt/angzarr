//! gRPC aggregate context.
//!
//! Uses EventBookRepository for storage and K8s service discovery for projectors.
//! Business logic invocation is handled by the pipeline via gRPC client.

use std::sync::Arc;

use async_trait::async_trait;
use tonic::Status;
use tracing::warn;
use uuid::Uuid;

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::proto::{EventBook, Projection, SyncEventBook};
use crate::repository::EventBookRepository;
use crate::services::snapshot_handler::persist_snapshot_if_present;
use crate::services::upcaster::Upcaster;
use crate::storage::{EventStore, SnapshotStore, StorageError};
use crate::utils::sequence_validator::sequence_mismatch_error_with_state;

use super::{AggregateContext, TemporalQuery};

/// gRPC aggregate context using EventBookRepository and K8s service discovery.
pub struct GrpcAggregateContext {
    event_store: Arc<dyn EventStore>,
    event_book_repo: Arc<EventBookRepository>,
    snapshot_store: Arc<dyn SnapshotStore>,
    discovery: Arc<dyn ServiceDiscovery>,
    event_bus: Arc<dyn EventBus>,
    upcaster: Option<Arc<Upcaster>>,
    snapshot_write_enabled: bool,
    /// When Some, call projectors synchronously with this mode.
    /// When None, only publish to event bus (async mode).
    sync_mode: Option<crate::proto::SyncMode>,
}

impl GrpcAggregateContext {
    /// Create a new gRPC aggregate context (async mode - no sync projectors).
    pub fn new(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        discovery: Arc<dyn ServiceDiscovery>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            event_store: Arc::clone(&event_store),
            event_book_repo: Arc::new(EventBookRepository::new(event_store, Arc::clone(&snapshot_store))),
            snapshot_store,
            discovery,
            event_bus,
            upcaster: None,
            snapshot_write_enabled: true,
            sync_mode: None,
        }
    }

    /// Create with configurable snapshot behavior.
    pub fn with_config(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        discovery: Arc<dyn ServiceDiscovery>,
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
            discovery,
            event_bus,
            upcaster: None,
            snapshot_write_enabled,
            sync_mode: None,
        }
    }

    /// Set the upcaster for event version transformation.
    pub fn with_upcaster(mut self, upcaster: Arc<Upcaster>) -> Self {
        self.upcaster = Some(upcaster);
        self
    }

    /// Set sync mode to call projectors synchronously.
    ///
    /// When set, post_persist will call projectors with this mode.
    /// When None (default), only publishes to event bus.
    pub fn with_sync_mode(mut self, mode: crate::proto::SyncMode) -> Self {
        self.sync_mode = Some(mode);
        self
    }

    /// Call sync projectors via K8s service discovery.
    async fn call_sync_projectors(
        &self,
        events: &EventBook,
        sync_mode: crate::proto::SyncMode,
    ) -> Result<Vec<Projection>, Status> {
        let clients = self.discovery.get_all_projectors().await.map_err(|e| {
            warn!(error = %e, "Failed to get projector coordinator clients");
            Status::unavailable(format!("Projector discovery failed: {e}"))
        })?;

        if clients.is_empty() {
            return Ok(vec![]);
        }

        let mut projections = Vec::new();
        for mut client in clients {
            let request = tonic::Request::new(SyncEventBook {
                events: Some(events.clone()),
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

#[async_trait]
impl AggregateContext for GrpcAggregateContext {
    async fn load_prior_events(
        &self,
        domain: &str,
        root: Uuid,
        temporal: &TemporalQuery,
    ) -> Result<EventBook, Status> {
        match temporal {
            TemporalQuery::Current => self
                .event_book_repo
                .get(domain, root)
                .await
                .map_err(|e| Status::internal(format!("Failed to load events: {e}"))),
            TemporalQuery::AsOfSequence(seq) => self
                .event_book_repo
                .get_temporal_by_sequence(domain, root, *seq)
                .await
                .map_err(|e| Status::internal(format!("Failed to load temporal events: {e}"))),
            TemporalQuery::AsOfTimestamp(ts) => self
                .event_book_repo
                .get_temporal_by_time(domain, root, ts)
                .await
                .map_err(|e| Status::internal(format!("Failed to load temporal events: {e}"))),
        }
    }

    async fn persist_events(
        &self,
        events: &EventBook,
        domain: &str,
        root: Uuid,
        _correlation_id: &str,
    ) -> Result<EventBook, Status> {
        // Persist events
        self.event_book_repo.put(events).await.map_err(|e| match e {
            StorageError::SequenceConflict { expected, actual } => Status::aborted(format!(
                "Sequence conflict: expected {}, got {}",
                expected, actual
            )),
            _ => Status::internal(format!("Failed to persist events: {e}")),
        })?;

        // Persist snapshot if present and enabled
        persist_snapshot_if_present(
            &self.snapshot_store,
            events,
            domain,
            root,
            self.snapshot_write_enabled,
        )
        .await?;

        Ok(events.clone())
    }

    async fn post_persist(&self, events: &EventBook) -> Result<Vec<Projection>, Status> {
        // Call sync projectors if sync_mode is set
        let projections = if let Some(mode) = self.sync_mode {
            self.call_sync_projectors(events, mode).await?
        } else {
            vec![]
        };

        // Publish to event bus for async consumers
        if let Err(e) = self.event_bus.publish(Arc::new(events.clone())).await {
            let domain = events
                .cover
                .as_ref()
                .map(|c| c.domain.as_str())
                .unwrap_or("unknown");
            warn!(
                domain = %domain,
                error = %e,
                "Failed to publish events"
            );
        }

        Ok(projections)
    }

    async fn pre_validate_sequence(
        &self,
        domain: &str,
        root: Uuid,
        expected: u32,
    ) -> Result<(), Status> {
        let next_sequence = self
            .event_store
            .get_next_sequence(domain, root)
            .await
            .map_err(|e| Status::internal(format!("Failed to get sequence: {e}")))?;

        if expected != next_sequence {
            // Load EventBook and return with error so caller can retry without extra fetch
            let prior_events = self
                .event_book_repo
                .get(domain, root)
                .await
                .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;
            return Err(sequence_mismatch_error_with_state(
                expected,
                next_sequence,
                &prior_events,
            ));
        }

        Ok(())
    }

    async fn transform_events(
        &self,
        domain: &str,
        mut events: EventBook,
    ) -> Result<EventBook, Status> {
        if let Some(ref upcaster) = self.upcaster {
            let upcasted_pages = upcaster
                .upcast(domain, events.pages)
                .await
                .map_err(|e| Status::internal(format!("Upcaster failed: {e}")))?;
            events.pages = upcasted_pages;
        }
        Ok(events)
    }
}
