//! Local (in-process) process manager context.
//!
//! Delegates prepare/handle to in-process `ProcessManagerHandler`.
//! Persists PM events directly to the event store and publishes to the bus.
//! PM events bypass the command pipeline, so this module passes the default
//! edition directly to storage.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::error;

use crate::bus::EventBus;
use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::orchestration::command::CommandOutcome;
use crate::proto::{CommandResponse, Cover, EventBook};
use crate::standalone::DomainStorage;
use crate::standalone::ProcessManagerHandler;

use super::{PMContextFactory, PmHandleResponse, ProcessManagerContext};

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
    async fn prepare(
        &self,
        trigger: &EventBook,
        pm_state: Option<&EventBook>,
    ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.handler.prepare(trigger, pm_state))
    }

    async fn handle(
        &self,
        trigger: &EventBook,
        pm_state: Option<&EventBook>,
        destinations: &[EventBook],
    ) -> Result<PmHandleResponse, Box<dyn std::error::Error + Send + Sync>> {
        let (commands, process_events) = self.handler.handle(trigger, pm_state, destinations);
        Ok(PmHandleResponse {
            commands,
            process_events,
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

        // PM events bypass the command pipeline, so we pass the default edition directly.
        // Persist to event store
        if let Err(e) = self
            .pm_store
            .event_store
            .add(
                &self.pm_domain,
                DEFAULT_EDITION,
                pm_root,
                process_events.pages.clone(),
                correlation_id,
            )
            .await
        {
            return CommandOutcome::Rejected(e.to_string());
        }

        // Re-read persisted events for publishing
        match self
            .pm_store
            .event_store
            .get(&self.pm_domain, DEFAULT_EDITION, pm_root)
            .await
        {
            Ok(pages) => {
                let full_book = EventBook {
                    cover: process_events.cover.clone(),
                    pages,
                    snapshot: None,
                    snapshot_state: None,
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
}

/// Factory that produces `LocalPMContext` instances for standalone mode.
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
