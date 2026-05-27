//! gRPC process manager context.
//!
//! Delegates handle to remote `ProcessManagerServiceClient` via gRPC.
//! Persists PM events directly to event store and publishes to event bus,
//! bypassing the command pipeline (no aggregate sidecar for PM domain).

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::error;

use crate::bus::EventBus;
use crate::dlq::DeadLetterPublisher;
use crate::orchestration::command::CommandOutcome;
use crate::proto::process_manager_service_client::ProcessManagerServiceClient;
use crate::proto::{CommandResponse, EventBook, ProcessManagerHandleRequest};
use crate::proto_ext::{correlated_request, CoverExt};
use crate::storage::EventStore;

use super::{PMContextFactory, PmHandleResponse, ProcessManagerContext};

/// Persist a PM event book to the event store and publish the
/// re-read result on the event bus.
///
/// Extracted from `GrpcPMContext::persist_pm_events` so tests can
/// exercise the persist path without constructing a full
/// `GrpcPMContext` (which requires a live
/// `ProcessManagerServiceClient` gRPC channel even for paths that
/// don't invoke `handle()`). Same shape as the aggregate's
/// `publish_aggregate_sequence_mismatch_dlq` refactor.
///
/// Returns:
/// - `CommandOutcome::Success` when both `event_store.add` and the
///   subsequent `event_store.get` + `event_bus.publish` succeed.
/// - `CommandOutcome::Rejected { code: Internal, ... }` when
///   `event_store.add` returns Err. The caller's retry loop in
///   `orchestrate_pm` classifies this as immediate-Rejected per
///   R2-15 (it does NOT count toward the retry budget). The bus
///   publish step never fails the persist outcome -- a failed
///   publish is logged but the events ARE durably persisted.
pub async fn persist_pm_event_book(
    event_store: &Arc<dyn EventStore>,
    event_bus: &Arc<dyn EventBus>,
    pm_domain: &str,
    process_events: &EventBook,
    correlation_id: &str,
) -> CommandOutcome {
    let pm_root = process_events
        .cover
        .as_ref()
        .and_then(|c| c.root.as_ref())
        .and_then(|r| uuid::Uuid::from_slice(&r.value).ok())
        .unwrap_or_else(uuid::Uuid::nil);
    let edition = process_events.edition().unwrap_or_default();

    // Persist directly to event store (bypasses command pipeline)
    if let Err(e) = event_store
        .add(
            pm_domain,
            edition,
            pm_root,
            process_events.pages.clone(),
            correlation_id,
            None, // No idempotency key for PM events
            None, // No source tracking for PM events
        )
        .await
    {
        // Event-store add failures here are server-side faults
        // (storage I/O, sequence races at the storage layer). The
        // caller doesn't go through the saga retry loop for this
        // path, so the Code is metadata for downstream DLQ
        // classification rather than a live retry signal.
        return CommandOutcome::Rejected {
            code: tonic::Code::Internal,
            message: e.to_string(),
        };
    }

    // Re-read persisted events for publishing
    match event_store.get(pm_domain, edition, pm_root).await {
        Ok(pages) => {
            let full_book = EventBook {
                cover: process_events.cover.clone(),
                pages,
                snapshot: None,
                ..Default::default()
            };
            if let Err(e) = event_bus.publish(Arc::new(full_book)).await {
                error!(
                    domain = %pm_domain,
                    error = %e,
                    "Failed to publish PM events"
                );
            }
        }
        Err(e) => {
            error!(
                domain = %pm_domain,
                error = %e,
                "Failed to re-read PM events for publishing"
            );
        }
    }

    CommandOutcome::Success(CommandResponse::default())
}

/// gRPC PM context that calls remote ProcessManager service.
///
/// Persists PM state events directly to the event store (no aggregate sidecar).
pub struct GrpcPMContext {
    client: Arc<Mutex<ProcessManagerServiceClient<tonic::transport::Channel>>>,
    event_store: Arc<dyn EventStore>,
    event_bus: Arc<dyn EventBus>,
    pm_domain: String,
    dlq_publisher: Arc<dyn DeadLetterPublisher>,
    component_name: String,
}

impl GrpcPMContext {
    /// Create with gRPC client, event store, event bus, and PM domain.
    pub fn new(
        client: Arc<Mutex<ProcessManagerServiceClient<tonic::transport::Channel>>>,
        event_store: Arc<dyn EventStore>,
        event_bus: Arc<dyn EventBus>,
        pm_domain: String,
        dlq_publisher: Arc<dyn DeadLetterPublisher>,
        component_name: String,
    ) -> Self {
        Self {
            client,
            event_store,
            event_bus,
            pm_domain,
            dlq_publisher,
            component_name,
        }
    }
}

#[async_trait]
impl ProcessManagerContext for GrpcPMContext {
    async fn handle(
        &self,
        trigger: &EventBook,
        pm_state: Option<&EventBook>,
    ) -> Result<PmHandleResponse, Box<dyn std::error::Error + Send + Sync>> {
        let correlation_id = trigger.correlation_id();

        tracing::info!(
            trigger_pages = trigger.pages.len(),
            trigger_has_snapshot = trigger.snapshot.is_some(),
            trigger_domain = %trigger.domain(),
            "GrpcPMContext.handle sending trigger to PM"
        );

        // PMs do not rebuild destination state; destination_sequences is
        // populated by the coordinator side from any pre-fetched aggregates.
        // For pure-PM-state PMs this map is empty.
        let request = ProcessManagerHandleRequest {
            trigger: Some(trigger.clone()),
            process_state: pm_state.cloned(),
            destination_sequences: Default::default(),
        };

        let mut client = self.client.lock().await;
        let response = client
            .handle(correlated_request(request, correlation_id))
            .await?
            .into_inner();

        let mut commands = response.commands;
        let mut process_events = response.process_events;
        let mut facts = response.facts;

        // Audit #86 contract: stamp the trigger cover's edition onto
        // every outgoing book. See `super::edition_propagation` for
        // the contract details and tests.
        super::edition_propagation::propagate_trigger_edition(
            trigger.cover.as_ref(),
            &mut commands,
            &mut process_events,
            &mut facts,
        );

        Ok(PmHandleResponse {
            commands,
            process_events,
            facts,
        })
    }

    async fn persist_pm_events(
        &self,
        process_events: &EventBook,
        correlation_id: &str,
    ) -> CommandOutcome {
        persist_pm_event_book(
            &self.event_store,
            &self.event_bus,
            &self.pm_domain,
            process_events,
            correlation_id,
        )
        .await
    }

    fn dlq_publisher(&self) -> Option<&Arc<dyn DeadLetterPublisher>> {
        Some(&self.dlq_publisher)
    }

    fn component_name(&self) -> &str {
        &self.component_name
    }
}

/// Factory that produces `GrpcPMContext` instances for distributed mode.
///
/// Captures long-lived gRPC client, event store, and event bus.
/// Each call to `create()` produces a context for one PM invocation.
pub struct GrpcPMContextFactory {
    client: Arc<Mutex<ProcessManagerServiceClient<tonic::transport::Channel>>>,
    event_store: Arc<dyn EventStore>,
    event_bus: Arc<dyn EventBus>,
    name: String,
    pm_domain: String,
    dlq_publisher: Arc<dyn DeadLetterPublisher>,
}

impl GrpcPMContextFactory {
    /// Create a new factory with gRPC client, event store, event bus, and PM domain.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client: Arc<Mutex<ProcessManagerServiceClient<tonic::transport::Channel>>>,
        event_store: Arc<dyn EventStore>,
        event_bus: Arc<dyn EventBus>,
        name: String,
        pm_domain: String,
        dlq_publisher: Arc<dyn DeadLetterPublisher>,
    ) -> Self {
        Self {
            client,
            event_store,
            event_bus,
            name,
            pm_domain,
            dlq_publisher,
        }
    }
}

impl PMContextFactory for GrpcPMContextFactory {
    fn create(&self) -> Box<dyn ProcessManagerContext> {
        Box::new(GrpcPMContext::new(
            self.client.clone(),
            self.event_store.clone(),
            self.event_bus.clone(),
            self.pm_domain.clone(),
            self.dlq_publisher.clone(),
            self.name.clone(),
        ))
    }

    fn pm_domain(&self) -> &str {
        &self.pm_domain
    }

    fn name(&self) -> &str {
        &self.name
    }
}
