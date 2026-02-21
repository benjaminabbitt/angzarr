//! Local (in-process) aggregate context.
//!
//! Uses SQLite-backed storage with static service discovery for projectors.
//! client logic invocation is handled by the pipeline via gRPC client.

use std::sync::Arc;

use async_trait::async_trait;
use tonic::Status;
use tracing::warn;
use uuid::Uuid;

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::dlq::{AngzarrDeadLetter, DeadLetterPublisher, NoopDeadLetterPublisher};
use crate::proto::{
    CommandBook, Cover, Edition, EventBook, EventPage, MergeStrategy, Projection, Snapshot,
    SyncEventBook, Uuid as ProtoUuid,
};
use crate::proto_ext::{calculate_set_next_seq, CoverExt};
use crate::standalone::DomainStorage;
use crate::storage::StorageError;

use super::{AggregateContext, TemporalQuery};

/// Build an EventBook with proper next_sequence set.
fn build_event_book(
    domain: &str,
    edition: &str,
    root: Uuid,
    pages: Vec<EventPage>,
    snapshot: Option<Snapshot>,
) -> EventBook {
    let mut book = EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: Some(Edition {
                name: edition.to_string(),
                divergences: vec![],
            }),
        }),
        pages,
        snapshot,
        ..Default::default()
    };
    calculate_set_next_seq(&mut book);
    book
}

/// Extract sequence number from an EventPage.
fn extract_sequence(page: Option<&crate::proto::EventPage>) -> u32 {
    page.map(|p| p.sequence).unwrap_or(0)
}

/// Local aggregate context using in-process storage with optional service discovery.
///
/// When `discovery` is `Some`, sync projectors are called after persist.
/// When `None` (edition mode), only publishes to the event bus.
pub struct LocalAggregateContext {
    storage: DomainStorage,
    discovery: Option<Arc<dyn ServiceDiscovery>>,
    event_bus: Arc<dyn EventBus>,
    snapshot_write_enabled: bool,
    dlq_publisher: Arc<dyn DeadLetterPublisher>,
    /// Component name for DLQ metadata.
    component_name: String,
}

impl LocalAggregateContext {
    /// Create a new local aggregate context with sync projector support.
    pub fn new(
        storage: DomainStorage,
        discovery: Arc<dyn ServiceDiscovery>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            storage,
            discovery: Some(discovery),
            event_bus,
            snapshot_write_enabled: true,
            dlq_publisher: Arc::new(NoopDeadLetterPublisher),
            component_name: "aggregate".to_string(),
        }
    }

    /// Create without service discovery (no sync projectors).
    pub fn without_discovery(storage: DomainStorage, event_bus: Arc<dyn EventBus>) -> Self {
        Self {
            storage,
            discovery: None,
            event_bus,
            snapshot_write_enabled: true,
            dlq_publisher: Arc::new(NoopDeadLetterPublisher),
            component_name: "aggregate".to_string(),
        }
    }

    /// Disable snapshot writing.
    pub fn with_snapshot_write_disabled(mut self) -> Self {
        self.snapshot_write_enabled = false;
        self
    }

    /// Set the DLQ publisher for MERGE_MANUAL handling.
    pub fn with_dlq_publisher(mut self, publisher: Arc<dyn DeadLetterPublisher>) -> Self {
        self.dlq_publisher = publisher;
        self
    }

    /// Set the component name for DLQ metadata.
    pub fn with_component_name(mut self, name: impl Into<String>) -> Self {
        self.component_name = name.into();
        self
    }

    /// Call sync projectors via service discovery.
    #[tracing::instrument(name = "aggregate.sync_projectors", skip_all)]
    async fn call_sync_projectors(&self, events: &EventBook) -> Vec<Projection> {
        let discovery = match &self.discovery {
            Some(d) => d,
            None => return vec![],
        };

        let clients = match discovery.get_all_projectors().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Failed to get projector clients");
                return vec![];
            }
        };

        let mut projections = Vec::new();
        for mut client in clients {
            let request = tonic::Request::new(SyncEventBook {
                events: Some(events.clone()),
                sync_mode: crate::proto::SyncMode::Simple.into(),
            });
            match client.handle_sync(request).await {
                Ok(response) => projections.push(response.into_inner()),
                Err(e) if e.code() == tonic::Code::NotFound => {
                    // Projector doesn't handle this domain - skip
                }
                Err(e) => {
                    warn!(error = %e, "Projector sync call failed");
                }
            }
        }

        projections
    }
}

#[async_trait]
impl AggregateContext for LocalAggregateContext {
    #[tracing::instrument(name = "aggregate.load_events", skip_all, fields(%domain, %root))]
    async fn load_prior_events(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        temporal: &TemporalQuery,
    ) -> Result<EventBook, Status> {
        match temporal {
            TemporalQuery::Current => {
                // Try to load snapshot first
                let snapshot = self
                    .storage
                    .snapshot_store
                    .get(domain, edition, root)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to load snapshot: {e}")))?;

                let (events, snapshot_data) = if let Some(snap) = snapshot {
                    let from_seq = snap.sequence + 1;
                    let events = self
                        .storage
                        .event_store
                        .get_from(domain, edition, root, from_seq)
                        .await
                        .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;
                    (events, Some(snap))
                } else {
                    let events = self
                        .storage
                        .event_store
                        .get(domain, edition, root)
                        .await
                        .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;
                    (events, None)
                };

                Ok(build_event_book(
                    domain,
                    edition,
                    root,
                    events,
                    snapshot_data,
                ))
            }
            TemporalQuery::AsOfSequence(seq) => {
                // Get events from 0 to sequence (inclusive)
                let events = self
                    .storage
                    .event_store
                    .get_from_to(domain, edition, root, 0, seq + 1)
                    .await
                    .map_err(|e| {
                        Status::internal(format!("Failed to load temporal events: {e}"))
                    })?;

                Ok(build_event_book(domain, edition, root, events, None))
            }
            TemporalQuery::AsOfTimestamp(ts) => {
                let events = self
                    .storage
                    .event_store
                    .get_until_timestamp(domain, edition, root, ts)
                    .await
                    .map_err(|e| {
                        Status::internal(format!("Failed to load temporal events: {e}"))
                    })?;

                Ok(build_event_book(domain, edition, root, events, None))
            }
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
        correlation_id: &str,
    ) -> Result<EventBook, Status> {
        // Compute new pages: those in received but not in prior
        let prior_max_seq = prior.pages.iter().map(|p| extract_sequence(Some(p))).max();
        let new_pages: Vec<_> = received
            .pages
            .iter()
            .filter(|p| {
                let seq = extract_sequence(Some(*p));
                prior_max_seq.is_none_or(|max| seq > max)
            })
            .cloned()
            .collect();

        // Check if snapshot changed (compare state bytes)
        let snapshot_changed = match (&prior.snapshot, &received.snapshot) {
            (None, Some(s)) => s.state.is_some(),
            (Some(_), None) => false, // Client cleared snapshot, don't persist
            (None, None) => false,
            (Some(p), Some(r)) => {
                // Compare state bytes
                let prior_state = p.state.as_ref().map(|s| &s.value);
                let received_state = r.state.as_ref().map(|s| &s.value);
                prior_state != received_state
            }
        };

        if new_pages.is_empty() && !snapshot_changed {
            // Nothing to persist (command was a no-op or returned identical state)
            let cover = received.cover.clone().map(|mut c| {
                if c.correlation_id.is_empty() {
                    c.correlation_id = correlation_id.to_string();
                }
                c
            });
            return Ok(EventBook {
                cover,
                pages: vec![],
                snapshot: received.snapshot.clone(),
                ..Default::default()
            });
        }

        // Persist new events if any
        if !new_pages.is_empty() {
            // Validate sequence
            let next_sequence = self
                .storage
                .event_store
                .get_next_sequence(domain, edition, root)
                .await
                .map_err(|e| Status::internal(format!("Failed to get sequence: {e}")))?;
            let first_event_seq = extract_sequence(new_pages.first());

            if first_event_seq != next_sequence {
                return Err(Status::failed_precondition(format!(
                    "Sequence conflict: expected {}, got {}",
                    next_sequence, first_event_seq
                )));
            }

            self.storage
                .event_store
                .add(domain, edition, root, new_pages.clone(), correlation_id)
                .await
                .map_err(|e| match e {
                    StorageError::SequenceConflict { expected, actual } => {
                        Status::failed_precondition(format!(
                            "Sequence conflict: expected {}, got {}",
                            expected, actual
                        ))
                    }
                    _ => Status::internal(format!("Failed to persist events: {e}")),
                })?;
        }

        // Persist snapshot if changed and enabled
        if self.snapshot_write_enabled && snapshot_changed {
            if let Some(ref snapshot) = received.snapshot {
                if let Some(ref state) = snapshot.state {
                    // Compute sequence from the last persisted event
                    let last_seq = new_pages
                        .last()
                        .map(|p| extract_sequence(Some(p)))
                        .or(prior_max_seq)
                        .unwrap_or(0);
                    let persisted_snapshot = crate::proto::Snapshot {
                        sequence: last_seq,
                        state: Some(state.clone()),
                        retention: crate::proto::SnapshotRetention::RetentionDefault as i32,
                    };
                    self.storage
                        .snapshot_store
                        .put(domain, edition, root, persisted_snapshot)
                        .await
                        .map_err(|e| {
                            Status::internal(format!("Failed to persist snapshot: {e}"))
                        })?;
                }
            }
        }

        // Return new events with correlation ID and edition set on cover
        let cover = received.cover.clone().map(|mut c| {
            if c.correlation_id.is_empty() {
                c.correlation_id = correlation_id.to_string();
            }
            c.edition = Some(Edition {
                name: edition.to_string(),
                divergences: vec![],
            });
            c
        });
        Ok(EventBook {
            cover,
            pages: new_pages,
            snapshot: received.snapshot.clone(),
            ..Default::default()
        })
    }

    #[tracing::instrument(name = "aggregate.post_persist", skip_all)]
    async fn post_persist(&self, events: &EventBook) -> Result<Vec<Projection>, Status> {
        // Call sync projectors
        let projections = self.call_sync_projectors(events).await;

        tracing::info!(
            pages = events.pages.len(),
            domain = %events.domain(),
            "Aggregate publishing events to bus"
        );

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
            let outcome = if publish_result.is_ok() {
                "success"
            } else {
                "error"
            };
            BUS_PUBLISH_DURATION.record(
                publish_start.elapsed().as_secs_f64(),
                &[
                    metrics::component_attr("aggregate"),
                    metrics::domain_attr(&routing_key),
                    metrics::outcome_attr(outcome),
                ],
            );
            BUS_PUBLISH_TOTAL.add(
                1,
                &[
                    metrics::component_attr("aggregate"),
                    metrics::domain_attr(&routing_key),
                    metrics::outcome_attr(outcome),
                ],
            );
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

    // Uses default pre_validate_sequence (no-op) — load-first strategy
    // Uses default transform_events (identity) — no upcasting in local mode

    async fn send_to_dlq(
        &self,
        command: &CommandBook,
        expected_sequence: u32,
        actual_sequence: u32,
        domain: &str,
    ) {
        let dead_letter = AngzarrDeadLetter::from_sequence_mismatch(
            command,
            expected_sequence,
            actual_sequence,
            MergeStrategy::MergeManual,
            &self.component_name,
        );

        if let Err(e) = self.dlq_publisher.publish(dead_letter).await {
            tracing::error!(
                domain = %domain,
                expected = expected_sequence,
                actual = actual_sequence,
                error = %e,
                "Failed to publish to DLQ"
            );
        }
    }
}
