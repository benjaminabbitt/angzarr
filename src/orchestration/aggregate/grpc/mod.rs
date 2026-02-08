//! gRPC aggregate context.
//!
//! Uses EventBookRepository for storage and K8s service discovery for projectors.
//! client logic invocation is handled by the pipeline via gRPC client.

use std::sync::Arc;

use async_trait::async_trait;
use tonic::Status;
use tracing::warn;
use uuid::Uuid;

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::proto::{EventBook, Projection, Snapshot, SyncEventBook};
use crate::proto_ext::{correlated_request, CoverExt, EventPageExt};
use crate::repository::EventBookRepository;
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
            event_book_repo: Arc::new(EventBookRepository::new(
                event_store,
                Arc::clone(&snapshot_store),
            )),
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
    #[tracing::instrument(name = "aggregate.sync_projectors", skip_all)]
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

        let correlation_id = events.correlation_id();
        let mut projections = Vec::new();
        for mut client in clients {
            let request = correlated_request(
                SyncEventBook {
                    events: Some(events.clone()),
                    sync_mode: sync_mode.into(),
                },
                correlation_id,
            );
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
    #[tracing::instrument(name = "aggregate.load_events", skip_all, fields(%domain, %root))]
    async fn load_prior_events(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        temporal: &TemporalQuery,
    ) -> Result<EventBook, Status> {
        match temporal {
            TemporalQuery::Current => self
                .event_book_repo
                .get(domain, edition, root)
                .await
                .map_err(|e| Status::internal(format!("Failed to load events: {e}"))),
            TemporalQuery::AsOfSequence(seq) => self
                .event_book_repo
                .get_temporal_by_sequence(domain, edition, root, *seq)
                .await
                .map_err(|e| Status::internal(format!("Failed to load temporal events: {e}"))),
            TemporalQuery::AsOfTimestamp(ts) => self
                .event_book_repo
                .get_temporal_by_time(domain, edition, root, ts)
                .await
                .map_err(|e| Status::internal(format!("Failed to load temporal events: {e}"))),
        }
    }

    #[tracing::instrument(name = "aggregate.persist", skip_all, fields(%domain, %root))]
    async fn persist_events(
        &self,
        prior: &EventBook,
        received: &EventBook,
        domain: &str,
        edition: &str,
        root: Uuid,
        _correlation_id: &str,
    ) -> Result<EventBook, Status> {
        // Compute new pages: those in received but not in prior
        let prior_max_seq = prior.pages.iter().map(|p| p.sequence_num()).max();
        let new_pages: Vec<_> = received
            .pages
            .iter()
            .filter(|p| {
                let seq = p.sequence_num();
                prior_max_seq.map_or(true, |max| seq > max)
            })
            .cloned()
            .collect();

        // Check if snapshot changed (compare state bytes)
        let snapshot_changed = match (&prior.snapshot, &received.snapshot) {
            (None, Some(s)) => s.state.is_some(),
            (Some(_), None) => false, // Client cleared snapshot, don't persist
            (None, None) => false,
            (Some(p), Some(r)) => {
                let prior_state = p.state.as_ref().map(|s| &s.value);
                let received_state = r.state.as_ref().map(|s| &s.value);
                prior_state != received_state
            }
        };

        if new_pages.is_empty() && !snapshot_changed {
            // Nothing to persist
            return Ok(received.clone());
        }

        // Persist new events if any
        if !new_pages.is_empty() {
            let events_to_persist = EventBook {
                cover: received.cover.clone(),
                pages: new_pages.clone(),
                snapshot: None,
            };
            self.event_book_repo
                .put(edition, &events_to_persist)
                .await
                .map_err(|e| match e {
                    StorageError::SequenceConflict { expected, actual } => Status::aborted(format!(
                        "Sequence conflict: expected {}, got {}",
                        expected, actual
                    )),
                    _ => Status::internal(format!("Failed to persist events: {e}")),
                })?;
        }

        // Persist snapshot if changed and enabled
        if self.snapshot_write_enabled && snapshot_changed {
            if let Some(ref snapshot) = received.snapshot {
                if let Some(ref state) = snapshot.state {
                    // Compute sequence from the last event
                    let last_seq = new_pages
                        .last()
                        .map(|p| p.sequence_num())
                        .or(prior_max_seq)
                        .unwrap_or(0);
                    let persisted_snapshot = Snapshot {
                        sequence: last_seq,
                        state: Some(state.clone()),
                    };
                    self.snapshot_store
                        .put(domain, edition, root, persisted_snapshot)
                        .await
                        .map_err(|e| Status::internal(format!("Failed to persist snapshot: {e}")))?;
                }
            }
        }

        // Return with only new pages
        Ok(EventBook {
            cover: received.cover.clone(),
            pages: new_pages,
            snapshot: received.snapshot.clone(),
        })
    }

    #[tracing::instrument(name = "aggregate.post_persist", skip_all)]
    async fn post_persist(&self, events: &EventBook) -> Result<Vec<Projection>, Status> {
        // Call sync projectors if sync_mode is set
        let projections = if let Some(mode) = self.sync_mode {
            self.call_sync_projectors(events, mode).await?
        } else {
            vec![]
        };

        // Publish events to bus — cover.domain stays bare, bus computes routing key
        #[cfg(feature = "otel")]
        let publish_start = std::time::Instant::now();

        let bus_events = Arc::new(events.clone());

        #[cfg(feature = "otel")]
        let routing_key = bus_events.routing_key();

        let publish_result = self.event_bus.publish(bus_events).await;

        #[cfg(feature = "otel")]
        {
            use crate::utils::metrics::{self, BUS_PUBLISH_DURATION, BUS_PUBLISH_TOTAL};
            let outcome = if publish_result.is_ok() { "success" } else { "error" };
            BUS_PUBLISH_DURATION.record(publish_start.elapsed().as_secs_f64(), &[
                metrics::component_attr("aggregate"),
                metrics::domain_attr(&routing_key),
                metrics::outcome_attr(outcome),
            ]);
            BUS_PUBLISH_TOTAL.add(1, &[
                metrics::component_attr("aggregate"),
                metrics::domain_attr(&routing_key),
                metrics::outcome_attr(outcome),
            ]);
        }

        if let Err(e) = publish_result {
            warn!(
                domain = %events.domain(),
                error = %e,
                "Failed to publish events"
            );
        }

        Ok(projections)
    }

    #[tracing::instrument(name = "aggregate.pre_validate", skip_all, fields(%domain, %root, %expected))]
    async fn pre_validate_sequence(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        expected: u32,
    ) -> Result<(), Status> {
        let next_sequence = self
            .event_store
            .get_next_sequence(domain, edition, root)
            .await
            .map_err(|e| Status::internal(format!("Failed to get sequence: {e}")))?;

        if expected != next_sequence {
            // Load EventBook and return with error so caller can retry without extra fetch
            let prior_events = self
                .event_book_repo
                .get(domain, edition, root)
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

    #[tracing::instrument(name = "aggregate.transform", skip_all, fields(%domain))]
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
