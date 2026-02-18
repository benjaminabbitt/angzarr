//! Local (in-process) process manager context.
//!
//! Delegates prepare/handle to in-process `ProcessManagerHandler`.
//! Persists PM events directly to the event store and publishes to the bus.
//! PM events bypass the command pipeline, so this module passes the default
//! edition directly to storage.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{error, info, warn};

use crate::bus::EventBus;
use crate::orchestration::command::CommandOutcome;
use crate::proto::{
    CommandBook, CommandResponse, Edition, EventBook, Notification, RejectionNotification,
};
use crate::proto_ext::{CoverExt, EditionExt};
use crate::standalone::DomainStorage;
use crate::standalone::ProcessManagerHandler;
use crate::utils::saga_compensation::CompensationContext;

use super::{PMContextFactory, PmHandleResponse, PmPrepareResponse, ProcessManagerContext};

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
    ) -> Result<PmPrepareResponse, Box<dyn std::error::Error + Send + Sync>> {
        let edition = trigger.edition().to_string();
        let mut covers = self.handler.prepare(trigger, pm_state);

        // Stamp trigger edition onto outgoing covers
        for cover in &mut covers {
            if cover.edition.as_ref().is_none_or(|e| e.is_empty()) {
                cover.edition = Some(Edition {
                    name: edition.clone(),
                    divergences: vec![],
                });
            }
        }
        Ok(PmPrepareResponse {
            destinations: covers,
        })
    }

    async fn handle(
        &self,
        trigger: &EventBook,
        pm_state: Option<&EventBook>,
        destinations: &[EventBook],
    ) -> Result<PmHandleResponse, Box<dyn std::error::Error + Send + Sync>> {
        let edition = trigger.edition().to_string();
        let (commands, process_events) = self.handler.handle(trigger, pm_state, destinations);

        // Stamp trigger edition onto outgoing command covers
        let commands = commands
            .into_iter()
            .map(|mut cmd| {
                if let Some(ref mut c) = cmd.cover {
                    if c.edition.as_ref().is_none_or(|e| e.is_empty()) {
                        c.edition = Some(Edition {
                            name: edition.clone(),
                            divergences: vec![],
                        });
                    }
                }
                cmd
            })
            .collect();

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
        let edition = process_events.edition();

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

    async fn on_command_rejected(&self, command: &CommandBook, reason: &str, correlation_id: &str) {
        // Build compensation context from rejected command
        let Some(context) = CompensationContext::from_rejected_command(command, reason.to_string())
        else {
            warn!(
                reason = %reason,
                "PM command rejected (not a saga command, no compensation context)"
            );
            return;
        };

        let saga_name = &context.saga_origin.saga_name;

        info!(
            saga = %saga_name,
            pm_domain = %self.pm_domain,
            reason = %reason,
            "PM command rejected, invoking handle_revocation"
        );

        // Build Notification with RejectionNotification payload
        let rejection = RejectionNotification {
            rejected_command: Some(command.clone()),
            rejection_reason: reason.to_string(),
            issuer_name: saga_name.clone(),
            issuer_type: "process_manager".to_string(),
            source_aggregate: context.saga_origin.triggering_aggregate.clone(),
            source_event_sequence: context.saga_origin.triggering_event_sequence,
        };

        let notification = Notification {
            cover: context.saga_origin.triggering_aggregate.clone(),
            payload: Some(prost_types::Any {
                type_url: "type.googleapis.com/angzarr.RejectionNotification".to_string(),
                value: prost::Message::encode_to_vec(&rejection),
            }),
            sent_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            metadata: std::collections::HashMap::new(),
        };

        // Load PM state by correlation_id
        let pm_root = uuid::Uuid::parse_str(correlation_id).unwrap_or_else(|_| uuid::Uuid::nil());
        let pm_state = self
            .pm_store
            .event_store
            .get(&self.pm_domain, "", pm_root)
            .await
            .ok()
            .map(|pages| EventBook {
                pages,
                ..Default::default()
            });

        // Call handler's revocation method
        let (pm_events, revocation_response) = self
            .handler
            .handle_revocation(&notification, pm_state.as_ref());

        // Persist any PM events
        if let Some(events) = pm_events {
            if !events.pages.is_empty() {
                match self.persist_pm_events(&events, correlation_id).await {
                    CommandOutcome::Success(_) => {
                        info!(
                            events = events.pages.len(),
                            "PM compensation events persisted"
                        );
                    }
                    CommandOutcome::Rejected(e) => {
                        error!(
                            error = %e,
                            "Failed to persist PM compensation events"
                        );
                    }
                    CommandOutcome::Retryable { reason, .. } => {
                        warn!(
                            reason = %reason,
                            "PM compensation events conflict (not retried)"
                        );
                    }
                }
            }
        }

        // Log framework action if requested
        if revocation_response.emit_system_revocation {
            info!(
                saga = %saga_name,
                reason = %revocation_response.reason,
                "PM requested system revocation event"
            );
        }
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
