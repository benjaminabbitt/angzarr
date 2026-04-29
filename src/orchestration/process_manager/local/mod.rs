//! Local (in-process) process manager context.
//!
//! Delegates handle to in-process `ProcessManagerHandler`.
//! Persists PM events directly to the event store and publishes to the bus.
//! PM events bypass the command pipeline, so this module passes the default
//! edition directly to storage.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{error, warn};

use crate::bus::EventBus;
use crate::orchestration::command::CommandOutcome;
use crate::proto::{CommandBook, CommandResponse, EventBook};
use crate::proto_ext::CoverExt;
use crate::storage::DomainStorage;

use super::{PMContextFactory, PmHandleResponse, ProcessManagerContext, ProcessManagerHandler};

/// Local PM context that calls in-process handler and persists to event store.
pub struct LocalPMContext {
    handler: Arc<dyn ProcessManagerHandler>,
    pm_domain: String,
    pm_store: DomainStorage,
    event_bus: Arc<dyn EventBus>,
}

impl LocalPMContext {
    /// Create with PM handler, domain storage, and event bus.
    pub fn new(
        handler: Arc<dyn ProcessManagerHandler>,
        pm_domain: String,
        pm_store: DomainStorage,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            handler,
            pm_domain,
            pm_store,
            event_bus,
        }
    }
}

#[async_trait]
impl ProcessManagerContext for LocalPMContext {
    async fn handle(
        &self,
        trigger: &EventBook,
        pm_state: Option<&EventBook>,
    ) -> Result<PmHandleResponse, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.handler.handle(trigger, pm_state);

        let mut commands = result.commands;
        let mut process_events = result.process_events;
        let mut facts = result.facts;

        // Audit #86 contract (coordinator-contract/edition_propagation.feature):
        // always-override propagation of trigger cover's edition onto
        // every outgoing book. Logic factored into the free helper
        // `propagate_trigger_edition` so it can be unit-tested without
        // standing up DomainStorage / EventBus.
        propagate_trigger_edition(
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
        let pm_root = process_events
            .cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .and_then(|r| uuid::Uuid::from_slice(&r.value).ok())
            .unwrap_or_else(uuid::Uuid::nil);
        let edition = process_events.edition().unwrap_or_default();

        // PM events bypass the command pipeline — use edition from trigger cover.
        // Persist to event store
        if let Err(e) = self
            .pm_store
            .event_store
            .add(
                &self.pm_domain,
                edition,
                pm_root,
                process_events.pages.clone(),
                correlation_id,
                None, // No idempotency key for PM events
                None, // No source tracking for PM events
            )
            .await
        {
            return CommandOutcome::Rejected(e.to_string());
        }

        // Re-read persisted events for publishing
        match self
            .pm_store
            .event_store
            .get(&self.pm_domain, edition, pm_root)
            .await
        {
            Ok(pages) => {
                let full_book = EventBook {
                    cover: process_events.cover.clone(),
                    pages,
                    snapshot: None,
                    ..Default::default()
                };
                if let Err(e) = self.event_bus.publish(Arc::new(full_book)).await {
                    error!(
                        domain = %self.pm_domain,
                        error = %e,
                        "Failed to publish PM events"
                    );
                }
            }
            Err(e) => {
                error!(
                    domain = %self.pm_domain,
                    error = %e,
                    "Failed to re-read PM events for publishing"
                );
            }
        }

        CommandOutcome::Success(CommandResponse::default())
    }

    async fn on_command_rejected(
        &self,
        _command: &CommandBook,
        reason: &str,
        _correlation_id: &str,
    ) {
        // Compensation now routes through CommandRouter to PM domain.
        // PM commands have angzarr_deferred.source stamped (pointing to PM), and PM domain
        // is registered in CommandRouter via ProcessManagerHandlerAdapter.
        // When the aggregate rejects, it creates a Notification command that
        // routes to the PM through the standard command infrastructure.
        //
        // This callback is now just for logging/metrics.
        warn!(
            pm_domain = %self.pm_domain,
            reason = %reason,
            "PM command rejected, compensation will route through command infrastructure"
        );
    }
}

/// Factory that produces `LocalPMContext` instances for in-process mode.
///
/// Captures in-process PM handler and domain storage.
/// Each call to `create()` produces a context for one PM invocation.
pub struct LocalPMContextFactory {
    handler: Arc<dyn ProcessManagerHandler>,
    name: String,
    pm_domain: String,
    pm_store: DomainStorage,
    event_bus: Arc<dyn EventBus>,
}

impl LocalPMContextFactory {
    /// Create a new factory with PM handler and domain dependencies.
    pub fn new(
        handler: Arc<dyn ProcessManagerHandler>,
        name: String,
        pm_domain: String,
        pm_store: DomainStorage,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            handler,
            name,
            pm_domain,
            pm_store,
            event_bus,
        }
    }
}

impl PMContextFactory for LocalPMContextFactory {
    fn create(&self) -> Box<dyn ProcessManagerContext> {
        Box::new(LocalPMContext::new(
            self.handler.clone(),
            self.pm_domain.clone(),
            self.pm_store.clone(),
            self.event_bus.clone(),
        ))
    }

    fn pm_domain(&self) -> &str {
        &self.pm_domain
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Audit #86 contract: stamp the trigger cover's edition (full struct
/// including divergences) onto every outgoing book — commands,
/// process_events, and facts. Always-override: handler-set editions
/// get overwritten so the coordinator guarantees timeline consistency
/// across cross-domain emissions. When the trigger has no cover, no
/// propagation runs (outgoing books keep whatever the handler set).
///
/// Free function so unit tests can drive it without standing up
/// `DomainStorage` / `EventBus`. Same logic mirrored in `grpc/mod.rs`.
pub(crate) fn propagate_trigger_edition(
    trigger_cover: Option<&crate::proto::Cover>,
    commands: &mut [CommandBook],
    process_events: &mut [EventBook],
    facts: &mut [EventBook],
) {
    let Some(trigger_cover) = trigger_cover else {
        return;
    };
    for cmd in commands.iter_mut() {
        if let Some(c) = &mut cmd.cover {
            c.propagate_edition_from(trigger_cover);
        }
    }
    for book in process_events.iter_mut() {
        if let Some(c) = &mut book.cover {
            c.propagate_edition_from(trigger_cover);
        }
    }
    for book in facts.iter_mut() {
        if let Some(c) = &mut book.cover {
            c.propagate_edition_from(trigger_cover);
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
