//! Local (in-process) aggregate context.
//!
//! Uses SQLite-backed storage with static service discovery for projectors.
//! Business logic invocation is handled by the pipeline via gRPC client.

use std::sync::Arc;

use async_trait::async_trait;
use tonic::Status;
use tracing::warn;
use uuid::Uuid;

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::proto::{event_page, Cover, EventBook, Projection, SyncEventBook, Uuid as ProtoUuid};
use crate::standalone::DomainStorage;
use crate::storage::StorageError;

use super::{AggregateContext, TemporalQuery};

/// Extract sequence number from an EventPage.
fn extract_sequence(page: Option<&crate::proto::EventPage>) -> u32 {
    page.and_then(|p| match &p.sequence {
        Some(event_page::Sequence::Num(n)) => Some(*n),
        _ => None,
    })
    .unwrap_or(0)
}

/// Local aggregate context using SQLite storage and static service discovery.
pub struct LocalAggregateContext {
    storage: DomainStorage,
    discovery: Arc<dyn ServiceDiscovery>,
    event_bus: Arc<dyn EventBus>,
    snapshot_write_enabled: bool,
}

impl LocalAggregateContext {
    /// Create a new local aggregate context.
    pub fn new(
        storage: DomainStorage,
        discovery: Arc<dyn ServiceDiscovery>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            storage,
            discovery,
            event_bus,
            snapshot_write_enabled: true,
        }
    }

    /// Disable snapshot writing.
    pub fn with_snapshot_write_disabled(mut self) -> Self {
        self.snapshot_write_enabled = false;
        self
    }

    /// Call sync projectors via service discovery.
    async fn call_sync_projectors(&self, events: &EventBook) -> Vec<Projection> {
        let clients = match self.discovery.get_all_projectors().await {
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
    async fn load_prior_events(
        &self,
        domain: &str,
        root: Uuid,
        temporal: &TemporalQuery,
    ) -> Result<EventBook, Status> {
        match temporal {
            TemporalQuery::Current => {
                // Try to load snapshot first
                let snapshot = self
                    .storage
                    .snapshot_store
                    .get(domain, root)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to load snapshot: {e}")))?;

                let (events, snapshot_data) = if let Some(snap) = snapshot {
                    let from_seq = snap.sequence + 1;
                    let events = self
                        .storage
                        .event_store
                        .get_from(domain, root, from_seq)
                        .await
                        .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;
                    (events, Some(snap))
                } else {
                    let events = self
                        .storage
                        .event_store
                        .get(domain, root)
                        .await
                        .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?;
                    (events, None)
                };

                Ok(EventBook {
                    cover: Some(Cover {
                        domain: domain.to_string(),
                        root: Some(ProtoUuid {
                            value: root.as_bytes().to_vec(),
                        }),
                        correlation_id: String::new(),
                    }),
                    pages: events,
                    snapshot: snapshot_data,
                    snapshot_state: None,
                })
            }
            TemporalQuery::AsOfSequence(seq) => {
                // Get events from 0 to sequence (inclusive)
                let events = self
                    .storage
                    .event_store
                    .get_from_to(domain, root, 0, seq + 1)
                    .await
                    .map_err(|e| {
                        Status::internal(format!("Failed to load temporal events: {e}"))
                    })?;

                Ok(EventBook {
                    cover: Some(Cover {
                        domain: domain.to_string(),
                        root: Some(ProtoUuid {
                            value: root.as_bytes().to_vec(),
                        }),
                        correlation_id: String::new(),
                    }),
                    pages: events,
                    snapshot: None,
                    snapshot_state: None,
                })
            }
            TemporalQuery::AsOfTimestamp(ts) => {
                let events = self
                    .storage
                    .event_store
                    .get_until_timestamp(domain, root, ts)
                    .await
                    .map_err(|e| {
                        Status::internal(format!("Failed to load temporal events: {e}"))
                    })?;

                Ok(EventBook {
                    cover: Some(Cover {
                        domain: domain.to_string(),
                        root: Some(ProtoUuid {
                            value: root.as_bytes().to_vec(),
                        }),
                        correlation_id: String::new(),
                    }),
                    pages: events,
                    snapshot: None,
                    snapshot_state: None,
                })
            }
        }
    }

    async fn persist_events(
        &self,
        events: &EventBook,
        domain: &str,
        root: Uuid,
        correlation_id: &str,
    ) -> Result<EventBook, Status> {
        if events.pages.is_empty() {
            // No events to persist (command was a no-op)
            let cover = events.cover.clone().map(|mut c| {
                if c.correlation_id.is_empty() {
                    c.correlation_id = correlation_id.to_string();
                }
                c
            });
            return Ok(EventBook {
                cover,
                pages: vec![],
                snapshot: None,
                snapshot_state: None,
            });
        }

        // Validate sequence
        let next_sequence = self
            .storage
            .event_store
            .get_next_sequence(domain, root)
            .await
            .map_err(|e| Status::internal(format!("Failed to get sequence: {e}")))?;
        let first_event_seq = extract_sequence(events.pages.first());

        if first_event_seq != next_sequence {
            // Map SequenceConflict to Aborted so retry logic can detect it
            return Err(Status::aborted(format!(
                "Sequence conflict: expected {}, got {}",
                next_sequence, first_event_seq
            )));
        }

        // Persist events
        self.storage
            .event_store
            .add(domain, root, events.pages.clone(), correlation_id)
            .await
            .map_err(|e| match e {
                StorageError::SequenceConflict { expected, actual } => Status::aborted(format!(
                    "Sequence conflict: expected {}, got {}",
                    expected, actual
                )),
                _ => Status::internal(format!("Failed to persist events: {e}")),
            })?;

        // Persist snapshot if present and enabled
        if self.snapshot_write_enabled {
            if let Some(ref snapshot_state) = events.snapshot_state {
                let last_seq = extract_sequence(events.pages.last());
                let snapshot = crate::proto::Snapshot {
                    sequence: last_seq,
                    state: Some(snapshot_state.clone()),
                };
                self.storage
                    .snapshot_store
                    .put(domain, root, snapshot)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to persist snapshot: {e}")))?;
            }
        }

        // Return events with correlation ID set on cover
        let cover = events.cover.clone().map(|mut c| {
            if c.correlation_id.is_empty() {
                c.correlation_id = correlation_id.to_string();
            }
            c
        });
        Ok(EventBook {
            cover,
            pages: events.pages.clone(),
            snapshot: None,
            snapshot_state: events.snapshot_state.clone(),
        })
    }

    async fn post_persist(&self, events: &EventBook) -> Result<Vec<Projection>, Status> {
        // Call sync projectors
        let projections = self.call_sync_projectors(events).await;

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

    // Uses default pre_validate_sequence (no-op) — load-first strategy
    // Uses default transform_events (identity) — no upcasting in local mode
}
