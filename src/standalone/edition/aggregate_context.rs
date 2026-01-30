//! Edition-aware aggregate context.
//!
//! Implements `AggregateContext` using `EditionEventStore` for storage
//! and publishes events with edition-prefixed domains on the event bus.
//! Edition is a column parameter on all AggregateContext methods. This
//! context ignores the incoming edition and uses its own `edition_name`
//! since the `EditionEventStore` handles the edition routing internally.

use std::sync::Arc;

use async_trait::async_trait;
use tonic::Status;
use tracing::warn;
use uuid::Uuid;

use crate::bus::EventBus;
use crate::orchestration::aggregate::{AggregateContext, TemporalQuery};
use crate::proto::{Cover, EventBook, Projection, Uuid as ProtoUuid};
use crate::storage::{EventStore, StorageError};

use super::event_store::EditionEventStore;

/// Aggregate context that reads/writes through an `EditionEventStore`
/// and publishes events with edition-prefixed domains on the event bus.
///
/// The `edition` parameter on trait methods is accepted for conformance
/// but ignored — this context always uses its own `edition_name`.
pub struct EditionAggregateContext {
    store: Arc<EditionEventStore>,
    event_bus: Arc<dyn EventBus>,
    edition_name: String,
}

impl EditionAggregateContext {
    /// Create a new edition aggregate context.
    pub fn new(
        store: Arc<EditionEventStore>,
        event_bus: Arc<dyn EventBus>,
        edition_name: String,
    ) -> Self {
        Self {
            store,
            event_bus,
            edition_name,
        }
    }
}

#[async_trait]
impl AggregateContext for EditionAggregateContext {
    async fn load_prior_events(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        temporal: &TemporalQuery,
    ) -> Result<EventBook, Status> {
        let events = match temporal {
            TemporalQuery::Current => self
                .store
                .get(domain, edition, root)
                .await
                .map_err(|e| Status::internal(format!("Failed to load events: {e}")))?,
            TemporalQuery::AsOfSequence(seq) => self
                .store
                .get_from_to(domain, edition, root, 0, seq + 1)
                .await
                .map_err(|e| Status::internal(format!("Failed to load temporal events: {e}")))?,
            TemporalQuery::AsOfTimestamp(ts) => self
                .store
                .get_until_timestamp(domain, edition, root, ts)
                .await
                .map_err(|e| Status::internal(format!("Failed to load temporal events: {e}")))?,
        };

        Ok(EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: Some(self.edition_name.clone()),
            }),
            pages: events,
            snapshot: None,
            snapshot_state: None,
        })
    }

    async fn persist_events(
        &self,
        events: &EventBook,
        domain: &str,
        edition: &str,
        root: Uuid,
        correlation_id: &str,
    ) -> Result<EventBook, Status> {
        use crate::proto::event_page;
        use crate::storage::EventStore;

        if events.pages.is_empty() {
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
            .store
            .get_next_sequence(domain, edition, root)
            .await
            .map_err(|e| Status::internal(format!("Failed to get sequence: {e}")))?;
        let first_event_seq = events
            .pages
            .first()
            .and_then(|p| match &p.sequence {
                Some(event_page::Sequence::Num(n)) => Some(*n),
                _ => None,
            })
            .unwrap_or(0);

        if first_event_seq != next_sequence {
            return Err(Status::aborted(format!(
                "Sequence conflict: expected {}, got {}",
                next_sequence, first_event_seq
            )));
        }

        // Persist to edition via EditionEventStore
        self.store
            .add(domain, edition, root, events.pages.clone(), correlation_id)
            .await
            .map_err(|e| match e {
                StorageError::SequenceConflict { expected, actual } => Status::aborted(format!(
                    "Sequence conflict: expected {}, got {}",
                    expected, actual
                )),
                _ => Status::internal(format!("Failed to persist events: {e}")),
            })?;

        let cover = events.cover.clone().map(|mut c| {
            if c.correlation_id.is_empty() {
                c.correlation_id = correlation_id.to_string();
            }
            c.edition = Some(self.edition_name.clone());
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
        let cover = events.cover.as_ref();
        let edition = cover
            .and_then(|c| c.edition.as_deref())
            .unwrap_or(&self.edition_name);
        let bare_domain = cover.map(|c| c.domain.as_str()).unwrap_or("unknown");
        let bus_domain = format!("{edition}.{bare_domain}");

        let mut bus_events = events.clone();
        if let Some(ref mut c) = bus_events.cover {
            c.domain = bus_domain.clone();
        }

        if let Err(e) = self.event_bus.publish(Arc::new(bus_events)).await {
            warn!(
                domain = %bus_domain,
                edition = %self.edition_name,
                error = %e,
                "Failed to publish edition events"
            );
        }

        // No sync projectors for editions (all async via bus)
        Ok(vec![])
    }
}
