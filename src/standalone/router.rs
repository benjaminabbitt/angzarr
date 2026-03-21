//! Command routing for standalone runtime.
//!
//! Dispatches commands to registered aggregate client logic.

use std::collections::HashMap;
use std::sync::Arc;

use tonic::Status;
use tracing::{debug, error, info, warn, Instrument};

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::dlq::{AngzarrDeadLetter, DeadLetterPublisher};
use crate::orchestration::aggregate::local::LocalAggregateContext;
use crate::orchestration::aggregate::TemporalQuery;
use crate::orchestration::aggregate::{
    execute_command_pipeline, execute_command_with_retry, parse_command_cover, AggregateContext,
    ClientLogic, PipelineMode,
};
use crate::orchestration::correlation;
use crate::orchestration::destination::local::LocalDestinationFetcher;
use crate::orchestration::destination::DestinationFetcher;
use crate::orchestration::shared::fetch_destinations;
use crate::orchestration::{FactExecutor, FactInjectionError};
use crate::proto::{
    business_response, BusinessResponse, CascadeError, CascadeErrorMode, CommandBook,
    CommandResponse, ContextualCommand,
};
use crate::proto_ext::{CoverExt, EventBookExt, EventPageExt};
use crate::services::gap_fill::{GapFiller, LocalEventSource, PositionStoreAdapter};
use crate::storage::{EventStore, PositionStore, SnapshotStore};
use crate::utils::retry::saga_backoff;

use super::traits::{ProcessManagerHandler, ProjectorHandler, SagaHandler};

/// Tracks executed commands during CASCADE for COMPENSATE mode.
///
/// When COMPENSATE error mode is active, we need to track all successfully
/// executed commands so they can be compensated if a later command fails.
#[derive(Default)]
struct CascadeTracker {
    /// Commands that executed successfully (in execution order).
    executed_commands: Vec<CommandBook>,
}

impl CascadeTracker {
    fn new() -> Self {
        Self::default()
    }

    /// Record a successfully executed command.
    fn record_success(&mut self, command: CommandBook) {
        self.executed_commands.push(command);
    }

    /// Get commands for compensation (reverse order).
    fn commands_for_compensation(&self) -> impl Iterator<Item = &CommandBook> {
        self.executed_commands.iter().rev()
    }
}

/// Per-domain storage.
#[derive(Clone)]
pub struct DomainStorage {
    /// Event store for this domain.
    pub event_store: Arc<dyn EventStore>,
    /// Snapshot store for this domain.
    pub snapshot_store: Arc<dyn SnapshotStore>,
}

impl DomainStorage {
    /// Create an EventBookRepository for this domain's stores.
    ///
    /// Consolidates the repeated pattern of creating repositories from
    /// event_store and snapshot_store Arcs.
    pub fn event_book_repo(&self) -> crate::repository::EventBookRepository {
        crate::repository::EventBookRepository::new(
            self.event_store.clone(),
            self.snapshot_store.clone(),
        )
    }
}

/// In-process sync projector entry for standalone mode.
#[derive(Clone)]
pub struct SyncProjectorEntry {
    /// Projector name for logging.
    pub name: String,
    /// Handler to call synchronously during command response.
    pub handler: Arc<dyn ProjectorHandler>,
}

/// In-process sync saga entry for standalone mode.
///
/// Sync sagas are called during CASCADE mode to ensure the entire
/// command chain completes before the original request returns.
#[derive(Clone)]
pub struct SyncSagaEntry {
    /// Saga name for logging.
    pub name: String,
    /// Handler to call synchronously during CASCADE mode.
    pub handler: Arc<dyn SagaHandler>,
    /// Source domain this saga subscribes to.
    pub source_domain: String,
}

/// In-process sync process manager entry for standalone mode.
///
/// Sync PMs are called during CASCADE mode to ensure cross-domain
/// workflows complete before the original request returns.
/// Unlike sagas, PMs subscribe to multiple domains and maintain state.
#[derive(Clone)]
pub struct SyncPMEntry {
    /// PM name for logging and checkpoint tracking.
    pub name: String,
    /// Handler to call synchronously during CASCADE mode.
    pub handler: Arc<dyn ProcessManagerHandler>,
    /// PM's own aggregate domain (for PM state storage).
    pub pm_domain: String,
    /// Subscriptions: which domains/event types this PM listens to.
    pub subscriptions: Vec<crate::descriptor::Target>,
}

/// Command router for standalone runtime.
///
/// Routes commands to registered aggregate client logic.
/// Each domain has its own isolated storage.
#[derive(Clone)]
pub struct CommandRouter {
    /// client logic implementations by domain.
    business: Arc<HashMap<String, Arc<dyn ClientLogic>>>,
    /// Per-domain storage.
    stores: Arc<HashMap<String, DomainStorage>>,
    /// Service discovery for projectors.
    discovery: Arc<dyn ServiceDiscovery>,
    /// Event bus for publishing.
    event_bus: Arc<dyn EventBus>,
    /// In-process sync projectors (called during command response).
    sync_projectors: Arc<Vec<SyncProjectorEntry>>,
    /// In-process sync sagas (called during CASCADE mode).
    sync_sagas: Arc<Vec<SyncSagaEntry>>,
    /// In-process sync process managers (called during CASCADE mode).
    sync_pms: Arc<Vec<SyncPMEntry>>,
    /// The name of the edition this router is operating within, if any.
    edition_name: Option<String>,
    /// Position store for handler checkpoint tracking.
    position_store: Arc<dyn PositionStore>,
    /// Optional DLQ publisher for DEAD_LETTER cascade error mode.
    dlq_publisher: Option<Arc<dyn DeadLetterPublisher>>,
}

impl CommandRouter {
    /// Create a new command router.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        business: HashMap<String, Arc<dyn ClientLogic>>,
        stores: HashMap<String, DomainStorage>,
        discovery: Arc<dyn ServiceDiscovery>,
        event_bus: Arc<dyn EventBus>,
        sync_projectors: Vec<SyncProjectorEntry>,
        sync_sagas: Vec<SyncSagaEntry>,
        sync_pms: Vec<SyncPMEntry>,
        edition_name: Option<String>,
        position_store: Arc<dyn PositionStore>,
    ) -> Self {
        let domains: Vec<_> = business.keys().cloned().collect();
        info!(
            domains = ?domains,
            sync_projectors = sync_projectors.len(),
            sync_sagas = sync_sagas.len(),
            sync_pms = sync_pms.len(),
            edition = ?edition_name,
            "Command router initialized"
        );

        Self {
            business: Arc::new(business),
            stores: Arc::new(stores),
            discovery,
            event_bus,
            sync_projectors: Arc::new(sync_projectors),
            sync_sagas: Arc::new(sync_sagas),
            sync_pms: Arc::new(sync_pms),
            edition_name,
            position_store,
            dlq_publisher: None,
        }
    }

    /// Set the DLQ publisher for DEAD_LETTER cascade error mode.
    ///
    /// Without a DLQ publisher, DEAD_LETTER mode behaves like CONTINUE
    /// (errors are collected but not sent to DLQ).
    pub fn with_dlq_publisher(mut self, publisher: Arc<dyn DeadLetterPublisher>) -> Self {
        self.dlq_publisher = Some(publisher);
        self
    }

    /// Get list of registered domains.
    pub fn domains(&self) -> Vec<&str> {
        self.business.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a domain has a registered handler.
    pub fn has_handler(&self, domain: &str) -> bool {
        self.business.contains_key(domain)
    }

    /// Get client logic for a specific domain.
    ///
    /// Returns the registered handler if one exists for the domain.
    pub fn get_client_logic(&self, domain: &str) -> Option<Arc<dyn ClientLogic>> {
        self.business.get(domain).cloned()
    }

    /// Get both business handler and storage for a domain.
    ///
    /// Consolidates the repeated pattern of fetching both resources with
    /// appropriate error messages. Returns references to avoid cloning.
    #[allow(clippy::result_large_err)] // Status is 176 bytes but only allocated on error paths
    fn get_domain_resources(
        &self,
        domain: &str,
    ) -> Result<(&Arc<dyn ClientLogic>, &DomainStorage), Status> {
        let business = self.business.get(domain).ok_or_else(|| {
            Status::not_found(format!("No handler registered for domain: {domain}"))
        })?;
        let storage = self.stores.get(domain).ok_or_else(|| {
            Status::not_found(format!("No storage configured for domain: {domain}"))
        })?;
        Ok((business, storage))
    }

    /// Create an aggregate context for command execution.
    ///
    /// Handles edition-aware context creation with optional sync mode.
    fn create_context(
        &self,
        storage: &DomainStorage,
        sync_mode: Option<crate::proto::SyncMode>,
    ) -> Arc<dyn AggregateContext> {
        self.create_context_with_cascade(storage, sync_mode, None)
    }

    /// Create an aggregate context with optional cascade_id for 2PC.
    ///
    /// When cascade_id is set, events are written with committed=false.
    fn create_context_with_cascade(
        &self,
        storage: &DomainStorage,
        sync_mode: Option<crate::proto::SyncMode>,
        cascade_id: Option<&str>,
    ) -> Arc<dyn AggregateContext> {
        let ctx = match &self.edition_name {
            Some(_) => {
                LocalAggregateContext::without_discovery(storage.clone(), self.event_bus.clone())
            }
            None => LocalAggregateContext::new(
                storage.clone(),
                self.discovery.clone(),
                self.event_bus.clone(),
            ),
        };

        let ctx = match sync_mode {
            Some(mode) => ctx.with_sync_mode(mode),
            None => ctx,
        };

        let ctx = match cascade_id {
            Some(id) => ctx.with_cascade_id(id),
            None => ctx,
        };

        Arc::new(ctx)
    }

    /// Execute a command and return the response.
    ///
    /// Validates the command's sequence against the aggregate's current sequence
    /// (optimistic concurrency check) before running client logic.
    pub async fn execute(&self, command_book: CommandBook) -> Result<CommandResponse, Status> {
        self.execute_inner(command_book).await
    }

    /// Execute a command from a saga or process manager.
    ///
    /// Validates command sequence against aggregate state for optimistic
    /// concurrency control. Sagas/PMs must stamp correct sequences on commands
    /// based on fetched destination state.
    pub async fn execute_command(
        &self,
        command_book: CommandBook,
    ) -> Result<CommandResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.execute_inner(command_book).await.map_err(Into::into)
    }

    /// Call in-process sync projectors and return their projections.
    async fn call_sync_projectors(
        &self,
        events: &crate::proto::EventBook,
    ) -> Vec<crate::proto::Projection> {
        use super::traits::ProjectionMode;
        use crate::proto_ext::CoverExt;

        // Skip infrastructure domains (underscore prefix) - matches async projector behavior
        let domain = events.domain();
        if domain.starts_with('_') {
            return Vec::new();
        }

        let mut projections = Vec::new();
        for entry in self.sync_projectors.iter() {
            match entry.handler.handle(events, ProjectionMode::Execute).await {
                Ok(projection) => projections.push(projection),
                Err(e) => {
                    warn!(
                        projector = %entry.name,
                        error = %e,
                        "Sync projector failed"
                    );
                }
            }
        }
        projections
    }

    /// Call in-process sync sagas for CASCADE mode.
    ///
    /// Executes sagas that subscribe to the source domain, fetches destinations,
    /// and recursively executes the resulting commands with CASCADE mode.
    ///
    /// Before calling each saga, fills any gaps in the EventBook relative to
    /// the saga's checkpoint. This ensures sagas receive complete history.
    /// After successful handling, updates the saga's checkpoint.
    ///
    /// Returns a BoxFuture to support async recursion (sagas trigger commands
    /// which trigger more sagas).
    ///
    /// Returns collected errors based on cascade_error_mode:
    /// - FAIL_FAST: Returns first error immediately (Vec will have at most 1)
    /// - CONTINUE/DEAD_LETTER: Continues and collects all errors
    fn call_sync_sagas<'a>(
        &'a self,
        events: &'a crate::proto::EventBook,
        cascade_error_mode: CascadeErrorMode,
    ) -> futures::future::BoxFuture<'a, Result<Vec<CascadeError>, Status>> {
        use futures::FutureExt;
        let source_domain = events.domain().to_string();
        let span = tracing::info_span!("router.sync_sagas", %source_domain);

        async move {
            let mut collected_errors: Vec<CascadeError> = Vec::new();
            let source_domain = events.domain();

            // Skip infrastructure domains
            if source_domain.starts_with('_') {
                return Ok(collected_errors);
            }

            // Find sagas subscribed to this domain
            let matching_sagas: Vec<_> = self
                .sync_sagas
                .iter()
                .filter(|s| s.source_domain == source_domain)
                .collect();

            if matching_sagas.is_empty() {
                return Ok(collected_errors);
            }

            let edition = events.edition().to_string();

            // Get storage for the source domain to fill gaps
            let storage = match self.stores.get(source_domain) {
                Some(s) => s,
                None => {
                    warn!(domain = %source_domain, "No storage for source domain, skipping saga gap-fill");
                    return Ok(collected_errors);
                }
            };

            let repo = Arc::new(storage.event_book_repo());
            let event_source = LocalEventSource::new(repo);

            // Extract root for checkpoint tracking
            let root = match events.cover.as_ref().and_then(|c| c.root.as_ref()) {
                Some(r) => r.value.clone(),
                None => {
                    warn!("EventBook missing root, skipping saga gap-fill");
                    return Ok(collected_errors);
                }
            };

            for entry in matching_sagas {
                // Create per-saga position store adapter with handler/domain/edition baked in
                let position_store = PositionStoreAdapter::new(
                    self.position_store.clone(),
                    &entry.name,
                    source_domain,
                    &edition,
                );

                // Create gap filler for this saga
                let gap_filler = GapFiller::new(position_store, event_source.clone());

                // Fill gaps in EventBook relative to this saga's checkpoint
                let filled_events = match gap_filler.fill_if_needed(events.clone()).await {
                    Ok(filled) => filled,
                    Err(e) => {
                        let error = CascadeError {
                            component_name: entry.name.clone(),
                            component_type: "saga".to_string(),
                            error_message: format!("Gap-fill failed: {}", e),
                            source_domain: source_domain.to_string(),
                        };
                        warn!(
                            saga = %entry.name,
                            error = %e,
                            "Failed to fill EventBook gaps for saga"
                        );
                        if cascade_error_mode == CascadeErrorMode::CascadeErrorFailFast {
                            return Err(Status::internal(format!("Saga {} gap-fill failed: {}", entry.name, e)));
                        }
                        collected_errors.push(error);
                        continue;
                    }
                };

                // Sagas are pure translators — just call handle with source events.
                // No destination fetching needed. Commands have angzarr_deferred,
                // framework stamps explicit sequences on delivery.
                let mut response = match entry.handler.handle(&filled_events).await {
                    Ok(r) => r,
                    Err(e) => {
                        let error = CascadeError {
                            component_name: entry.name.clone(),
                            component_type: "saga".to_string(),
                            error_message: format!("Handle failed: {}", e),
                            source_domain: source_domain.to_string(),
                        };
                        warn!(
                            saga = %entry.name,
                            error = %e,
                            "Sync saga handle failed"
                        );
                        if cascade_error_mode == CascadeErrorMode::CascadeErrorFailFast {
                            return Err(e);
                        }
                        collected_errors.push(error);
                        continue;
                    }
                };

                // Update checkpoint after successful handling (only if we had events)
                if let Some(last_page) = filled_events.last_page() {
                    let max_seq = last_page.sequence_num();
                    if let Err(e) = gap_filler.update_checkpoint(&root, max_seq).await {
                        warn!(
                            saga = %entry.name,
                            sequence = max_seq,
                            error = %e,
                            "Failed to update saga checkpoint"
                        );
                    }
                }

                // Stamp edition on commands
                for cmd in &mut response.commands {
                    if let Some(c) = &mut cmd.cover {
                        c.stamp_edition_if_empty(&edition);
                    }
                }

                debug!(
                    saga = %entry.name,
                    commands = response.commands.len(),
                    checkpoint = ?filled_events.last_page().map(|p| p.sequence_num()),
                    "Sync saga produced commands"
                );

                // Recursively execute commands with CASCADE mode
                for command in response.commands {
                    match self.execute_with_cascade_internal(command, cascade_error_mode).await {
                        Ok(cmd_response) => {
                            // Propagate errors from recursive calls
                            collected_errors.extend(cmd_response.cascade_errors);
                        }
                        Err(e) => {
                            let error = CascadeError {
                                component_name: entry.name.clone(),
                                component_type: "saga".to_string(),
                                error_message: format!("Command execution failed: {}", e),
                                source_domain: source_domain.to_string(),
                            };
                            warn!(
                                saga = %entry.name,
                                error = %e,
                                "Sync saga command execution failed"
                            );
                            if cascade_error_mode == CascadeErrorMode::CascadeErrorFailFast {
                                return Err(e);
                            }
                            collected_errors.push(error);
                        }
                    }
                }
            }

            Ok(collected_errors)
        }
        .instrument(span)
        .boxed()
    }

    /// Call in-process sync process managers for CASCADE mode.
    ///
    /// PMs subscribe to multiple domains. For each PM:
    /// 1. Check if trigger domain matches any subscription
    /// 2. Extract correlation_id (required for PMs)
    /// 3. Gap-fill trigger EventBook for this PM
    /// 4. Fetch PM state by correlation_id (sync fetch, no gap-fill needed)
    /// 5. Call prepare() to get additional destinations
    /// 6. Fetch destinations (sync fetch, no gap-fill needed)
    /// 7. Call handle() to get commands + PM events + facts
    /// 8. Persist PM events (via PM domain storage)
    /// 9. Execute commands with CASCADE mode
    /// 10. Inject facts
    /// 11. Update trigger checkpoint
    ///
    /// Returns collected errors based on cascade_error_mode:
    /// - FAIL_FAST: Returns first error immediately
    /// - CONTINUE/DEAD_LETTER: Continues and collects all errors
    fn call_sync_pms<'a>(
        &'a self,
        events: &'a crate::proto::EventBook,
        cascade_error_mode: CascadeErrorMode,
    ) -> futures::future::BoxFuture<'a, Result<Vec<CascadeError>, Status>> {
        use futures::FutureExt;
        let trigger_domain = events.domain().to_string();
        let span = tracing::info_span!("router.sync_pms", %trigger_domain);

        async move {
            let mut collected_errors: Vec<CascadeError> = Vec::new();
            let trigger_domain = events.domain();

            // Skip infrastructure domains
            if trigger_domain.starts_with('_') {
                return Ok(collected_errors);
            }

            // PMs require correlation_id
            let correlation_id = match events.correlation_id() {
                id if !id.is_empty() => id.to_string(),
                _ => {
                    debug!("EventBook missing correlation_id, skipping PM processing");
                    return Ok(collected_errors);
                }
            };

            // Find PMs subscribed to this domain
            let matching_pms: Vec<_> = self
                .sync_pms
                .iter()
                .filter(|pm| {
                    pm.subscriptions.iter().any(|sub| sub.domain == trigger_domain)
                })
                .collect();

            if matching_pms.is_empty() {
                return Ok(collected_errors);
            }

            let edition = events.edition().to_string();

            // Get storage for the trigger domain to fill gaps
            let trigger_storage = match self.stores.get(trigger_domain) {
                Some(s) => s,
                None => {
                    warn!(domain = %trigger_domain, "No storage for trigger domain, skipping PM gap-fill");
                    return Ok(collected_errors);
                }
            };

            let repo = Arc::new(trigger_storage.event_book_repo());
            let event_source = LocalEventSource::new(repo);

            // Extract root for checkpoint tracking
            let trigger_root = match events.cover.as_ref().and_then(|c| c.root.as_ref()) {
                Some(r) => r.value.clone(),
                None => {
                    warn!("EventBook missing root, skipping PM gap-fill");
                    return Ok(collected_errors);
                }
            };

            // Create fetcher for PM state and destinations
            // Clone the inner HashMap from Arc for LocalDestinationFetcher
            let fetcher = LocalDestinationFetcher::new((*self.stores).clone());

            for entry in matching_pms {
                // Create per-PM position store adapter for trigger gap-filling
                let position_store = PositionStoreAdapter::new(
                    self.position_store.clone(),
                    &entry.name,
                    trigger_domain,
                    &edition,
                );

                // Create gap filler for trigger events
                let gap_filler = GapFiller::new(position_store, event_source.clone());

                // Gap-fill trigger EventBook relative to this PM's checkpoint
                let filled_trigger = match gap_filler.fill_if_needed(events.clone()).await {
                    Ok(filled) => filled,
                    Err(e) => {
                        let error = CascadeError {
                            component_name: entry.name.clone(),
                            component_type: "process_manager".to_string(),
                            error_message: format!("Gap-fill failed: {}", e),
                            source_domain: trigger_domain.to_string(),
                        };
                        warn!(
                            pm = %entry.name,
                            error = %e,
                            "Failed to fill EventBook gaps for PM"
                        );
                        if cascade_error_mode == CascadeErrorMode::CascadeErrorFailFast {
                            return Err(Status::internal(format!("PM {} gap-fill failed: {}", entry.name, e)));
                        }
                        collected_errors.push(error);
                        continue;
                    }
                };

                // Fetch PM state by correlation_id (sync fetch, no gap-fill needed)
                let pm_state = fetcher.fetch_by_correlation(&entry.pm_domain, &correlation_id).await;

                // Phase 1: Prepare — PM declares additional destinations
                let destination_covers = entry.handler.prepare(&filled_trigger, pm_state.as_ref());

                // Stamp edition on destination covers
                let destination_covers: Vec<_> = destination_covers
                    .into_iter()
                    .map(|mut cover| {
                        cover.stamp_edition_if_empty(&edition);
                        cover
                    })
                    .collect();

                // Fetch destinations (sync fetch, no gap-fill needed)
                let destinations = fetch_destinations(&fetcher, &destination_covers, &correlation_id).await;

                // Phase 2: Handle — produce commands + PM events + facts
                let result = entry.handler.handle(&filled_trigger, pm_state.as_ref(), &destinations);

                debug!(
                    pm = %entry.name,
                    commands = result.commands.len(),
                    has_process_events = result.process_events.is_some(),
                    facts = result.facts.len(),
                    "Sync PM produced output"
                );

                // Persist PM events before executing commands (crash recovery invariant)
                if let Some(ref process_events) = result.process_events {
                    if !process_events.pages.is_empty() {
                        let pm_storage = match self.stores.get(&entry.pm_domain) {
                            Some(s) => s,
                            None => {
                                let error = CascadeError {
                                    component_name: entry.name.clone(),
                                    component_type: "process_manager".to_string(),
                                    error_message: format!("No storage for PM domain {}", entry.pm_domain),
                                    source_domain: trigger_domain.to_string(),
                                };
                                warn!(pm_domain = %entry.pm_domain, "No storage for PM domain");
                                if cascade_error_mode == CascadeErrorMode::CascadeErrorFailFast {
                                    return Err(Status::internal(format!("No storage for PM domain {}", entry.pm_domain)));
                                }
                                collected_errors.push(error);
                                continue;
                            }
                        };

                        // PM root = correlation_id as UUID
                        let pm_root = uuid::Uuid::parse_str(&correlation_id)
                            .unwrap_or_else(|_| uuid::Uuid::nil());

                        if let Err(e) = pm_storage
                            .event_store
                            .add(
                                &entry.pm_domain,
                                &edition,
                                pm_root,
                                process_events.pages.clone(),
                                &correlation_id,
                                None, // No idempotency key
                                None, // No source tracking
                            )
                            .await
                        {
                            let error = CascadeError {
                                component_name: entry.name.clone(),
                                component_type: "process_manager".to_string(),
                                error_message: format!("Failed to persist PM events: {}", e),
                                source_domain: trigger_domain.to_string(),
                            };
                            warn!(
                                pm = %entry.name,
                                error = %e,
                                "Failed to persist PM events"
                            );
                            if cascade_error_mode == CascadeErrorMode::CascadeErrorFailFast {
                                return Err(Status::internal(format!("PM {} persist failed: {}", entry.name, e)));
                            }
                            collected_errors.push(error);
                            continue;
                        }

                        info!(
                            pm = %entry.name,
                            events = process_events.pages.len(),
                            "PM events persisted"
                        );
                    }
                }

                // Execute commands with CASCADE mode
                let mut commands = result.commands;
                for cmd in &mut commands {
                    if let Some(c) = &mut cmd.cover {
                        c.stamp_edition_if_empty(&edition);
                    }
                }

                for command in commands {
                    match self.execute_with_cascade_internal(command, cascade_error_mode).await {
                        Ok(cmd_response) => {
                            // Propagate errors from recursive calls
                            collected_errors.extend(cmd_response.cascade_errors);
                        }
                        Err(e) => {
                            let error = CascadeError {
                                component_name: entry.name.clone(),
                                component_type: "process_manager".to_string(),
                                error_message: format!("Command execution failed: {}", e),
                                source_domain: trigger_domain.to_string(),
                            };
                            warn!(
                                pm = %entry.name,
                                error = %e,
                                "Sync PM command execution failed"
                            );
                            if cascade_error_mode == CascadeErrorMode::CascadeErrorFailFast {
                                return Err(e);
                            }
                            collected_errors.push(error);
                        }
                    }
                }

                // Inject facts
                for fact in result.facts {
                    let fact_domain = fact
                        .cover
                        .as_ref()
                        .map(|c| c.domain.as_str())
                        .unwrap_or("unknown");
                    debug!(pm = %entry.name, domain = %fact_domain, "Injecting fact from PM");

                    match self.inject_fact(fact, true).await {
                        Ok(_) => {}
                        Err(e) => {
                            let error = CascadeError {
                                component_name: entry.name.clone(),
                                component_type: "process_manager".to_string(),
                                error_message: format!("Fact injection failed: {}", e),
                                source_domain: trigger_domain.to_string(),
                            };
                            warn!(
                                pm = %entry.name,
                                error = %e,
                                "PM fact injection failed"
                            );
                            if cascade_error_mode == CascadeErrorMode::CascadeErrorFailFast {
                                return Err(Status::internal(format!("PM {} fact injection failed: {}", entry.name, e)));
                            }
                            collected_errors.push(error);
                        }
                    }
                }

                // Update trigger checkpoint after successful handling
                if let Some(last_page) = filled_trigger.last_page() {
                    let max_seq = last_page.sequence_num();
                    if let Err(e) = gap_filler.update_checkpoint(&trigger_root, max_seq).await {
                        warn!(
                            pm = %entry.name,
                            sequence = max_seq,
                            error = %e,
                            "Failed to update PM trigger checkpoint"
                        );
                    }
                }
            }

            Ok(collected_errors)
        }
        .instrument(span)
        .boxed()
    }

    /// Call sync sagas with command tracking for COMPENSATE mode.
    ///
    /// Same as `call_sync_sagas` but uses CONTINUE semantics and tracks
    /// executed commands in the provided tracker.
    async fn call_sync_sagas_tracked(
        &self,
        events: &crate::proto::EventBook,
        tracker: &mut CascadeTracker,
    ) -> Result<Vec<CascadeError>, Status> {
        let mut collected_errors: Vec<CascadeError> = Vec::new();
        let source_domain = events.domain();

        // Skip infrastructure domains
        if source_domain.starts_with('_') {
            return Ok(collected_errors);
        }

        // Find sagas subscribed to this domain
        let matching_sagas: Vec<_> = self
            .sync_sagas
            .iter()
            .filter(|s| s.source_domain == source_domain)
            .collect();

        if matching_sagas.is_empty() {
            return Ok(collected_errors);
        }

        let edition = events.edition().to_string();
        let storage = match self.stores.get(source_domain) {
            Some(s) => s,
            None => {
                warn!(domain = %source_domain, "No storage for source domain, skipping saga gap-fill");
                return Ok(collected_errors);
            }
        };

        let repo = Arc::new(storage.event_book_repo());
        let event_source = LocalEventSource::new(repo);
        let root = match events.cover.as_ref().and_then(|c| c.root.as_ref()) {
            Some(r) => r.value.clone(),
            None => {
                warn!("EventBook missing root, skipping saga gap-fill");
                return Ok(collected_errors);
            }
        };

        for entry in matching_sagas {
            let position_store = PositionStoreAdapter::new(
                self.position_store.clone(),
                &entry.name,
                source_domain,
                &edition,
            );
            let gap_filler = GapFiller::new(position_store, event_source.clone());

            let filled_events = match gap_filler.fill_if_needed(events.clone()).await {
                Ok(filled) => filled,
                Err(e) => {
                    collected_errors.push(CascadeError {
                        component_name: entry.name.clone(),
                        component_type: "saga".to_string(),
                        error_message: format!("Gap-fill failed: {}", e),
                        source_domain: source_domain.to_string(),
                    });
                    continue;
                }
            };

            let mut response = match entry.handler.handle(&filled_events).await {
                Ok(r) => r,
                Err(e) => {
                    collected_errors.push(CascadeError {
                        component_name: entry.name.clone(),
                        component_type: "saga".to_string(),
                        error_message: format!("Handle failed: {}", e),
                        source_domain: source_domain.to_string(),
                    });
                    continue;
                }
            };

            if let Some(last_page) = filled_events.last_page() {
                let max_seq = last_page.sequence_num();
                if let Err(e) = gap_filler.update_checkpoint(&root, max_seq).await {
                    warn!(saga = %entry.name, sequence = max_seq, error = %e, "Failed to update saga checkpoint");
                }
            }

            for cmd in &mut response.commands {
                if let Some(c) = &mut cmd.cover {
                    c.stamp_edition_if_empty(&edition);
                }
            }

            // Recursively execute with tracking
            for command in response.commands {
                match self.execute_with_cascade_tracked(command, tracker).await {
                    Ok(cmd_response) => {
                        collected_errors.extend(cmd_response.cascade_errors);
                    }
                    Err(e) => {
                        collected_errors.push(CascadeError {
                            component_name: entry.name.clone(),
                            component_type: "saga".to_string(),
                            error_message: format!("Command execution failed: {}", e),
                            source_domain: source_domain.to_string(),
                        });
                    }
                }
            }
        }

        Ok(collected_errors)
    }

    /// Call sync PMs with command tracking for COMPENSATE mode.
    ///
    /// Same as `call_sync_pms` but uses CONTINUE semantics and tracks
    /// executed commands in the provided tracker.
    async fn call_sync_pms_tracked(
        &self,
        events: &crate::proto::EventBook,
        tracker: &mut CascadeTracker,
    ) -> Result<Vec<CascadeError>, Status> {
        let mut collected_errors: Vec<CascadeError> = Vec::new();
        let trigger_domain = events.domain();

        // Skip infrastructure domains
        if trigger_domain.starts_with('_') {
            return Ok(collected_errors);
        }

        // PMs require correlation_id
        let correlation_id = match events.correlation_id() {
            id if !id.is_empty() => id.to_string(),
            _ => {
                debug!("EventBook missing correlation_id, skipping PM processing");
                return Ok(collected_errors);
            }
        };

        // Find PMs subscribed to this domain
        let matching_pms: Vec<_> = self
            .sync_pms
            .iter()
            .filter(|pm| {
                pm.subscriptions
                    .iter()
                    .any(|sub| sub.domain == trigger_domain)
            })
            .collect();

        if matching_pms.is_empty() {
            return Ok(collected_errors);
        }

        let edition = events.edition().to_string();

        // Get storage for the trigger domain to fill gaps
        let trigger_storage = match self.stores.get(trigger_domain) {
            Some(s) => s,
            None => {
                warn!(domain = %trigger_domain, "No storage for trigger domain, skipping PM gap-fill");
                return Ok(collected_errors);
            }
        };

        let repo = Arc::new(trigger_storage.event_book_repo());
        let event_source = LocalEventSource::new(repo);

        // Extract root for checkpoint tracking
        let trigger_root = match events.cover.as_ref().and_then(|c| c.root.as_ref()) {
            Some(r) => r.value.clone(),
            None => {
                warn!("EventBook missing root, skipping PM gap-fill");
                return Ok(collected_errors);
            }
        };

        // Create fetcher for PM state and destinations
        let fetcher = LocalDestinationFetcher::new((*self.stores).clone());

        for entry in matching_pms {
            // Create per-PM position store adapter for trigger gap-filling
            let position_store = PositionStoreAdapter::new(
                self.position_store.clone(),
                &entry.name,
                trigger_domain,
                &edition,
            );

            // Create gap filler for trigger events
            let gap_filler = GapFiller::new(position_store, event_source.clone());

            // Gap-fill trigger EventBook relative to this PM's checkpoint
            let filled_trigger = match gap_filler.fill_if_needed(events.clone()).await {
                Ok(filled) => filled,
                Err(e) => {
                    collected_errors.push(CascadeError {
                        component_name: entry.name.clone(),
                        component_type: "process_manager".to_string(),
                        error_message: format!("Gap-fill failed: {}", e),
                        source_domain: trigger_domain.to_string(),
                    });
                    continue;
                }
            };

            // Fetch PM state by correlation_id (sync fetch, no gap-fill needed)
            let pm_state = fetcher
                .fetch_by_correlation(&entry.pm_domain, &correlation_id)
                .await;

            // Phase 1: Prepare — PM declares additional destinations (SYNC call, no await)
            let destination_covers = entry.handler.prepare(&filled_trigger, pm_state.as_ref());

            // Stamp edition on destination covers
            let destination_covers: Vec<_> = destination_covers
                .into_iter()
                .map(|mut cover| {
                    cover.stamp_edition_if_empty(&edition);
                    cover
                })
                .collect();

            // Fetch destinations (sync fetch, no gap-fill needed)
            let destinations =
                fetch_destinations(&fetcher, &destination_covers, &correlation_id).await;

            // Phase 2: Handle — produce commands + PM events + facts (SYNC call, no await)
            let result = entry
                .handler
                .handle(&filled_trigger, pm_state.as_ref(), &destinations);

            debug!(
                pm = %entry.name,
                commands = result.commands.len(),
                has_process_events = result.process_events.is_some(),
                facts = result.facts.len(),
                "Sync PM (tracked) produced output"
            );

            // Persist PM events before executing commands (crash recovery invariant)
            if let Some(ref process_events) = result.process_events {
                if !process_events.pages.is_empty() {
                    let pm_storage = match self.stores.get(&entry.pm_domain) {
                        Some(s) => s,
                        None => {
                            collected_errors.push(CascadeError {
                                component_name: entry.name.clone(),
                                component_type: "process_manager".to_string(),
                                error_message: format!(
                                    "No storage for PM domain {}",
                                    entry.pm_domain
                                ),
                                source_domain: trigger_domain.to_string(),
                            });
                            continue;
                        }
                    };

                    // PM root = correlation_id as UUID
                    let pm_root = uuid::Uuid::parse_str(&correlation_id)
                        .unwrap_or_else(|_| uuid::Uuid::nil());

                    if let Err(e) = pm_storage
                        .event_store
                        .add(
                            &entry.pm_domain,
                            &edition,
                            pm_root,
                            process_events.pages.clone(),
                            &correlation_id,
                            None, // No idempotency key
                            None, // No source tracking
                        )
                        .await
                    {
                        collected_errors.push(CascadeError {
                            component_name: entry.name.clone(),
                            component_type: "process_manager".to_string(),
                            error_message: format!("Failed to persist PM events: {}", e),
                            source_domain: trigger_domain.to_string(),
                        });
                        continue;
                    }

                    info!(
                        pm = %entry.name,
                        events = process_events.pages.len(),
                        "PM events persisted (tracked)"
                    );
                }
            }

            // Execute commands with CASCADE mode + tracking
            let mut commands = result.commands;
            for cmd in &mut commands {
                if let Some(c) = &mut cmd.cover {
                    c.stamp_edition_if_empty(&edition);
                }
            }

            for command in commands {
                match self.execute_with_cascade_tracked(command, tracker).await {
                    Ok(cmd_response) => {
                        // Propagate errors from recursive calls
                        collected_errors.extend(cmd_response.cascade_errors);
                    }
                    Err(e) => {
                        collected_errors.push(CascadeError {
                            component_name: entry.name.clone(),
                            component_type: "process_manager".to_string(),
                            error_message: format!("Command execution failed: {}", e),
                            source_domain: trigger_domain.to_string(),
                        });
                    }
                }
            }

            // Inject facts
            for fact in result.facts {
                let fact_domain = fact
                    .cover
                    .as_ref()
                    .map(|c| c.domain.as_str())
                    .unwrap_or("unknown");
                debug!(pm = %entry.name, domain = %fact_domain, "Injecting fact from PM (tracked)");

                if let Err(e) = self.inject_fact(fact, true).await {
                    collected_errors.push(CascadeError {
                        component_name: entry.name.clone(),
                        component_type: "process_manager".to_string(),
                        error_message: format!("Fact injection failed: {}", e),
                        source_domain: trigger_domain.to_string(),
                    });
                }
            }

            // Update trigger checkpoint after successful handling
            if let Some(last_page) = filled_trigger.last_page() {
                let max_seq = last_page.sequence_num();
                if let Err(e) = gap_filler.update_checkpoint(&trigger_root, max_seq).await {
                    warn!(
                        pm = %entry.name,
                        sequence = max_seq,
                        error = %e,
                        "Failed to update PM trigger checkpoint"
                    );
                }
            }
        }

        Ok(collected_errors)
    }

    /// Execute a command with CASCADE mode (sync projectors + sync sagas + sync PMs).
    ///
    /// Used by sync sagas to recursively execute their output commands.
    /// Also used by `LocalCommandExecutor` when receiving a CASCADE command.
    /// Uses FAIL_FAST error mode by default.
    #[tracing::instrument(name = "router.execute_cascade", skip_all, fields(domain = %command_book.domain()))]
    pub async fn execute_with_cascade(
        &self,
        command_book: CommandBook,
    ) -> Result<CommandResponse, Status> {
        self.execute_with_cascade_internal(command_book, CascadeErrorMode::CascadeErrorFailFast)
            .await
    }

    /// Execute a command atomically with 2PC (two-phase commit).
    ///
    /// All events are written with `committed=false` and grouped by a shared cascade_id.
    /// On success, writes Confirmation events to make them visible.
    /// On failure, writes Revocation events to mark them as NoOp.
    ///
    /// This enables atomic commit/rollback across multiple aggregates when sagas
    /// produce cross-domain commands.
    #[tracing::instrument(name = "router.execute_atomic", skip_all, fields(domain = %command_book.domain()))]
    pub async fn execute_atomic(
        &self,
        command_book: CommandBook,
    ) -> Result<CommandResponse, Status> {
        let cascade_id = uuid::Uuid::new_v4().to_string();
        self.execute_atomic_with_cascade_id(command_book, &cascade_id)
            .await
    }

    /// Execute atomically with a specific cascade_id.
    ///
    /// Internal method used by execute_atomic and for recursive saga calls.
    #[tracing::instrument(name = "router.execute_atomic_internal", skip_all, fields(domain = %command_book.domain(), %cascade_id))]
    async fn execute_atomic_with_cascade_id(
        &self,
        command_book: CommandBook,
        cascade_id: &str,
    ) -> Result<CommandResponse, Status> {
        let (domain, root_uuid) = parse_command_cover(&command_book)?;
        let (business, storage) = self.get_domain_resources(&domain)?;

        // Create context with cascade_id - events will be written with committed=false
        let ctx = self.create_context_with_cascade(
            storage,
            Some(crate::proto::SyncMode::Cascade),
            Some(cascade_id),
        );

        // Execute command - events are persisted with committed=false
        let response = match execute_command_with_retry(
            &*ctx,
            &**business,
            command_book,
            saga_backoff(),
        )
        .await
        {
            Ok(resp) => resp,
            Err(e) => {
                // Command failed - no events were persisted, no rollback needed
                return Err(e);
            }
        };

        // Track the sequences written by this command for commit/rollback
        let sequences: Vec<u32> = response
            .events
            .as_ref()
            .map(|eb| eb.pages.iter().map(|p| p.sequence_num()).collect())
            .unwrap_or_default();

        // CASCADE: call sync projectors (projectors see uncommitted events)
        let mut response = response;
        if !self.sync_projectors.is_empty() {
            if let Some(ref events) = response.events {
                let projections = self.call_sync_projectors(events).await;
                response.projections.extend(projections);
            }
        }

        // CASCADE: call sync sagas (recursive) with same cascade_id
        // TODO: For full 2PC, sagas should also use atomic execution with same cascade_id
        if !self.sync_sagas.is_empty() {
            if let Some(ref events) = response.events {
                match self
                    .call_sync_sagas(events, CascadeErrorMode::CascadeErrorFailFast)
                    .await
                {
                    Ok(saga_errors) => {
                        if !saga_errors.is_empty() {
                            // Saga produced errors - rollback
                            self.write_revocation(
                                &domain,
                                root_uuid,
                                cascade_id,
                                &sequences,
                                "saga_error",
                            )
                            .await;
                            return Err(Status::aborted(format!(
                                "Saga errors during atomic execution: {:?}",
                                saga_errors
                            )));
                        }
                    }
                    Err(e) => {
                        // Saga failed - rollback
                        self.write_revocation(
                            &domain,
                            root_uuid,
                            cascade_id,
                            &sequences,
                            "saga_failed",
                        )
                        .await;
                        return Err(e);
                    }
                }
            }
        }

        // CASCADE: call sync process managers
        if !self.sync_pms.is_empty() {
            if let Some(ref events) = response.events {
                match self
                    .call_sync_pms(events, CascadeErrorMode::CascadeErrorFailFast)
                    .await
                {
                    Ok(pm_errors) => {
                        if !pm_errors.is_empty() {
                            // PM produced errors - rollback
                            self.write_revocation(
                                &domain, root_uuid, cascade_id, &sequences, "pm_error",
                            )
                            .await;
                            return Err(Status::aborted(format!(
                                "PM errors during atomic execution: {:?}",
                                pm_errors
                            )));
                        }
                    }
                    Err(e) => {
                        // PM failed - rollback
                        self.write_revocation(
                            &domain,
                            root_uuid,
                            cascade_id,
                            &sequences,
                            "pm_failed",
                        )
                        .await;
                        return Err(e);
                    }
                }
            }
        }

        // All succeeded - write Confirmation to commit events
        if !sequences.is_empty() {
            self.write_confirmation(&domain, root_uuid, cascade_id, &sequences)
                .await;
        }

        Ok(response)
    }

    /// Write a Confirmation event to commit pending events.
    async fn write_confirmation(
        &self,
        domain: &str,
        root: uuid::Uuid,
        cascade_id: &str,
        sequences: &[u32],
    ) {
        use crate::proto::page_header::SequenceType;
        use crate::proto::{
            event_page, Confirmation, Cover, EventPage, PageHeader, Uuid as ProtoUuid,
        };
        use prost::Message;
        use prost_types::Any;

        let storage = match self.get_storage(domain) {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "Failed to get storage for confirmation");
                return;
            }
        };

        let edition = self.edition_name.as_deref().unwrap_or("");
        let next_seq = match storage
            .event_store
            .get_next_sequence(domain, edition, root)
            .await
        {
            Ok(seq) => seq,
            Err(e) => {
                error!(error = %e, "Failed to get next sequence for confirmation");
                return;
            }
        };

        let confirmation = Confirmation {
            target: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: None,
            }),
            sequences: sequences.to_vec(),
            cascade_id: cascade_id.to_string(),
        };

        let confirmation_page = EventPage {
            header: Some(PageHeader {
                sequence_type: Some(SequenceType::Sequence(next_seq)),
            }),
            created_at: None,
            payload: Some(event_page::Payload::Event(Any {
                type_url: "type.googleapis.com/angzarr.Confirmation".to_string(),
                value: confirmation.encode_to_vec(),
            })),
            committed: true, // Framework events are always committed
            cascade_id: None,
        };

        if let Err(e) = storage
            .event_store
            .add(
                domain,
                edition,
                root,
                vec![confirmation_page],
                "",
                None,
                None,
            )
            .await
        {
            error!(error = %e, "Failed to write confirmation event");
        }
    }

    /// Write a Revocation event to rollback pending events.
    async fn write_revocation(
        &self,
        domain: &str,
        root: uuid::Uuid,
        cascade_id: &str,
        sequences: &[u32],
        reason: &str,
    ) {
        use crate::proto::page_header::SequenceType;
        use crate::proto::{
            event_page, Cover, EventPage, PageHeader, Revocation, Uuid as ProtoUuid,
        };
        use prost::Message;
        use prost_types::Any;

        let storage = match self.get_storage(domain) {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "Failed to get storage for revocation");
                return;
            }
        };

        let edition = self.edition_name.as_deref().unwrap_or("");
        let next_seq = match storage
            .event_store
            .get_next_sequence(domain, edition, root)
            .await
        {
            Ok(seq) => seq,
            Err(e) => {
                error!(error = %e, "Failed to get next sequence for revocation");
                return;
            }
        };

        let revocation = Revocation {
            target: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: None,
            }),
            sequences: sequences.to_vec(),
            cascade_id: cascade_id.to_string(),
            reason: reason.to_string(),
        };

        let revocation_page = EventPage {
            header: Some(PageHeader {
                sequence_type: Some(SequenceType::Sequence(next_seq)),
            }),
            created_at: None,
            payload: Some(event_page::Payload::Event(Any {
                type_url: "type.googleapis.com/angzarr.Revocation".to_string(),
                value: revocation.encode_to_vec(),
            })),
            committed: true, // Framework events are always committed
            cascade_id: None,
        };

        if let Err(e) = storage
            .event_store
            .add(domain, edition, root, vec![revocation_page], "", None, None)
            .await
        {
            error!(error = %e, "Failed to write revocation event");
        }
    }

    /// Write a Compensate marker event for sequences that need compensation.
    ///
    /// Unlike Revocation, Compensate keeps original events visible and signals
    /// to client handlers that they should emit inverse events.
    async fn write_compensate(
        &self,
        domain: &str,
        root: uuid::Uuid,
        sequences: &[u32],
        reason: &str,
    ) {
        use crate::proto::page_header::SequenceType;
        use crate::proto::{
            event_page, Compensate, Cover, EventPage, PageHeader, Uuid as ProtoUuid,
        };
        use prost::Message;
        use prost_types::Any;

        let storage = match self.get_storage(domain) {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "Failed to get storage for compensate");
                return;
            }
        };

        let edition = self.edition_name.as_deref().unwrap_or("");
        let next_seq = match storage
            .event_store
            .get_next_sequence(domain, edition, root)
            .await
        {
            Ok(seq) => seq,
            Err(e) => {
                error!(error = %e, "Failed to get next sequence for compensate");
                return;
            }
        };

        let compensate = Compensate {
            target: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: None,
            }),
            sequences: sequences.to_vec(),
            reason: reason.to_string(),
        };

        let compensate_page = EventPage {
            header: Some(PageHeader {
                sequence_type: Some(SequenceType::Sequence(next_seq)),
            }),
            created_at: None,
            payload: Some(event_page::Payload::Event(Any {
                type_url: crate::proto_ext::type_url::COMPENSATE.to_string(),
                value: compensate.encode_to_vec(),
            })),
            committed: true, // Framework events are always committed
            cascade_id: None,
        };

        if let Err(e) = storage
            .event_store
            .add(domain, edition, root, vec![compensate_page], "", None, None)
            .await
        {
            error!(error = %e, "Failed to write compensate event");
        }
    }

    // =========================================================================
    // Cascade Coordination (Phase 6: PM 2PC)
    // =========================================================================

    /// Handle a CascadeCommit message from a Process Manager.
    ///
    /// Queries all participants in the cascade and writes Confirmation events
    /// for each one, making their uncommitted events visible to business logic.
    ///
    /// This is used by Process Managers coordinating async 2PC workflows.
    #[tracing::instrument(name = "router.handle_cascade_commit", skip_all, fields(%cascade_id))]
    pub async fn handle_cascade_commit(&self, cascade_id: &str) -> Result<usize, Status> {
        let mut confirmed_count = 0;

        // Query all participants across all domains
        for (domain, storage) in self.stores.iter() {
            let participants = match storage
                .event_store
                .query_cascade_participants(cascade_id)
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        domain = %domain,
                        error = %e,
                        "Failed to query cascade participants"
                    );
                    continue;
                }
            };

            for participant in participants {
                if !participant.sequences.is_empty() {
                    self.write_confirmation(
                        &participant.domain,
                        participant.root,
                        cascade_id,
                        &participant.sequences,
                    )
                    .await;
                    confirmed_count += 1;
                    debug!(
                        cascade_id = %cascade_id,
                        domain = %participant.domain,
                        root = ?participant.root,
                        sequences = ?participant.sequences,
                        "Confirmed cascade participant"
                    );
                }
            }
        }

        info!(
            cascade_id = %cascade_id,
            participants = confirmed_count,
            "Cascade committed"
        );

        Ok(confirmed_count)
    }

    /// Handle a CascadeRollback message from a Process Manager.
    ///
    /// Queries all participants in the cascade and writes Revocation events
    /// for each one, marking their uncommitted events as NoOp.
    ///
    /// This is used by Process Managers to rollback async 2PC workflows
    /// on failure or timeout.
    #[tracing::instrument(name = "router.handle_cascade_rollback", skip_all, fields(%cascade_id, %reason))]
    pub async fn handle_cascade_rollback(
        &self,
        cascade_id: &str,
        reason: &str,
    ) -> Result<usize, Status> {
        let mut revoked_count = 0;

        // Query all participants across all domains
        for (domain, storage) in self.stores.iter() {
            let participants = match storage
                .event_store
                .query_cascade_participants(cascade_id)
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        domain = %domain,
                        error = %e,
                        "Failed to query cascade participants"
                    );
                    continue;
                }
            };

            for participant in participants {
                if !participant.sequences.is_empty() {
                    self.write_revocation(
                        &participant.domain,
                        participant.root,
                        cascade_id,
                        &participant.sequences,
                        reason,
                    )
                    .await;
                    revoked_count += 1;
                    debug!(
                        cascade_id = %cascade_id,
                        domain = %participant.domain,
                        root = ?participant.root,
                        sequences = ?participant.sequences,
                        reason = %reason,
                        "Revoked cascade participant"
                    );
                }
            }
        }

        info!(
            cascade_id = %cascade_id,
            participants = revoked_count,
            reason = %reason,
            "Cascade rolled back"
        );

        Ok(revoked_count)
    }

    // =========================================================================
    // Revocation API (Phase 7)
    // =========================================================================

    /// Revoke committed events for an aggregate.
    ///
    /// This is a direct API for revoking any committed events, not just cascade
    /// participants. Revoked events become invisible to business logic (replaced
    /// with NoOp during read-time transformation).
    ///
    /// # Arguments
    /// * `domain` - Domain name of the aggregate
    /// * `root` - Aggregate root UUID
    /// * `sequences` - Event sequences to revoke (must be committed)
    /// * `reason` - Why the events are being revoked
    ///
    /// # Errors
    /// Returns error if:
    /// - Domain not found
    /// - Any sequence doesn't exist or isn't effectively committed
    #[tracing::instrument(name = "router.revoke_events", skip_all, fields(%domain, ?root, ?sequences, %reason))]
    pub async fn revoke_events(
        &self,
        domain: &str,
        root: uuid::Uuid,
        sequences: Vec<u32>,
        reason: &str,
    ) -> Result<(), Status> {
        if sequences.is_empty() {
            return Ok(());
        }

        let storage = self.get_storage(domain)?;
        let edition = self.edition_name.as_deref().unwrap_or("");

        // Load all events to validate sequences exist and are committed
        let events = storage
            .event_store
            .get(domain, edition, root)
            .await
            .map_err(|e| Status::internal(format!("Failed to load events: {}", e)))?;

        // Build set of confirmed sequences (from Confirmation events)
        let confirmed_sequences: std::collections::HashSet<u32> = events
            .iter()
            .filter_map(|e| {
                if let Some(crate::proto::event_page::Payload::Event(any)) = &e.payload {
                    if any.type_url.contains("Confirmation") {
                        // Parse Confirmation to get sequences
                        if let Ok(conf) =
                            <crate::proto::Confirmation as prost::Message>::decode(&any.value[..])
                        {
                            return Some(conf.sequences);
                        }
                    }
                }
                None
            })
            .flatten()
            .collect();

        // Validate all requested sequences are effectively committed
        for seq in &sequences {
            let event = events
                .iter()
                .find(|e| e.sequence_num() == *seq)
                .ok_or_else(|| {
                    Status::not_found(format!("Sequence {} not found in aggregate", seq))
                })?;

            let is_effectively_committed = event.committed || confirmed_sequences.contains(seq);

            if !is_effectively_committed {
                return Err(Status::failed_precondition(format!(
                    "Sequence {} is not committed and cannot be revoked. Use cascade rollback for uncommitted events.",
                    seq
                )));
            }
        }

        // Write Revocation event (with empty cascade_id since this is direct revocation)
        self.write_revocation(domain, root, "", &sequences, reason)
            .await;

        info!(
            domain = %domain,
            root = ?root,
            sequences = ?sequences,
            reason = %reason,
            "Events revoked"
        );

        Ok(())
    }

    /// Trigger compensation for specific committed events.
    ///
    /// Unlike `revoke_events()`, this keeps the original events visible in the
    /// event stream and writes a Compensate marker. Client handlers should
    /// implement inverse logic when they receive the Compensate event.
    ///
    /// # Parameters
    /// - `domain`: The domain of the aggregate
    /// - `root`: The aggregate root ID
    /// - `sequences`: The sequence numbers to compensate
    /// - `reason`: Human-readable reason for compensation
    ///
    /// # Returns
    /// - `Ok(())` if compensation marker was written
    /// - `Err(Status)` if sequences don't exist or are uncommitted
    ///
    /// # Differences from Revocation
    /// - **Revocation**: Original events become NoOp (hidden from business logic)
    /// - **Compensate**: Original events remain visible, handler emits inverse events
    #[tracing::instrument(
        name = "router.compensate_events",
        skip_all,
        fields(domain = %domain, root = ?root, sequences = ?sequences)
    )]
    pub async fn compensate_events(
        &self,
        domain: &str,
        root: uuid::Uuid,
        sequences: Vec<u32>,
        reason: &str,
    ) -> Result<(), Status> {
        if sequences.is_empty() {
            return Ok(());
        }

        let storage = self.get_storage(domain)?;
        let edition = self.edition_name.as_deref().unwrap_or("");

        // Load all events to validate sequences exist and are committed
        let events = storage
            .event_store
            .get(domain, edition, root)
            .await
            .map_err(|e| Status::internal(format!("Failed to load events: {}", e)))?;

        // Build set of confirmed sequences (from Confirmation events)
        let confirmed_sequences: std::collections::HashSet<u32> = events
            .iter()
            .filter_map(|e| {
                if let Some(crate::proto::event_page::Payload::Event(any)) = &e.payload {
                    if any.type_url.contains("Confirmation") {
                        if let Ok(conf) =
                            <crate::proto::Confirmation as prost::Message>::decode(&any.value[..])
                        {
                            return Some(conf.sequences);
                        }
                    }
                }
                None
            })
            .flatten()
            .collect();

        // Validate all requested sequences are effectively committed
        for seq in &sequences {
            let event = events
                .iter()
                .find(|e| e.sequence_num() == *seq)
                .ok_or_else(|| {
                    Status::not_found(format!("Sequence {} not found in aggregate", seq))
                })?;

            let is_effectively_committed = event.committed || confirmed_sequences.contains(seq);

            if !is_effectively_committed {
                return Err(Status::failed_precondition(format!(
                    "Sequence {} is not committed and cannot be compensated. Use cascade rollback for uncommitted events.",
                    seq
                )));
            }
        }

        // Write Compensate marker event
        self.write_compensate(domain, root, &sequences, reason)
            .await;

        info!(
            domain = %domain,
            root = ?root,
            sequences = ?sequences,
            reason = %reason,
            "Compensation requested for events"
        );

        Ok(())
    }

    /// Execute a command with CASCADE mode with specified error handling mode.
    ///
    /// Used when clients specify a cascade_error_mode other than FAIL_FAST.
    /// - FAIL_FAST: Stop on first error, fail request (default)
    /// - CONTINUE: Continue through all, return successes + errors
    /// - DEAD_LETTER: On error, send to DLQ and continue
    /// - COMPENSATE: On first error, compensate executed commands, fail request
    #[tracing::instrument(name = "router.execute_cascade_with_error_mode", skip_all, fields(domain = %command_book.domain()))]
    pub async fn execute_cascade_with_error_mode(
        &self,
        command_book: CommandBook,
        cascade_error_mode: CascadeErrorMode,
    ) -> Result<CommandResponse, Status> {
        match cascade_error_mode {
            CascadeErrorMode::CascadeErrorCompensate => {
                self.execute_with_compensate(command_book).await
            }
            CascadeErrorMode::CascadeErrorDeadLetter => {
                self.execute_with_dead_letter(command_book).await
            }
            _ => {
                self.execute_with_cascade_internal(command_book, cascade_error_mode)
                    .await
            }
        }
    }

    /// Execute CASCADE with DEAD_LETTER error mode.
    ///
    /// Uses CONTINUE semantics (don't fail fast). After execution, publishes
    /// any cascade errors to the DLQ, then returns success with errors included.
    #[tracing::instrument(name = "router.execute_with_dead_letter", skip_all, fields(domain = %command_book.domain()))]
    async fn execute_with_dead_letter(
        &self,
        command_book: CommandBook,
    ) -> Result<CommandResponse, Status> {
        // Execute with CONTINUE mode to collect all errors
        let response = self
            .execute_with_cascade_internal(
                command_book.clone(),
                CascadeErrorMode::CascadeErrorContinue,
            )
            .await?;

        // Publish any cascade errors to DLQ
        if !response.cascade_errors.is_empty() {
            self.publish_cascade_errors_to_dlq(&command_book, &response.cascade_errors)
                .await;
        }

        Ok(response)
    }

    /// Publish cascade errors to the Dead Letter Queue.
    ///
    /// Creates an AngzarrDeadLetter for each cascade error and publishes it.
    /// Errors during DLQ publishing are logged but don't fail the request.
    async fn publish_cascade_errors_to_dlq(
        &self,
        original_command: &CommandBook,
        cascade_errors: &[CascadeError],
    ) {
        let dlq_publisher = match &self.dlq_publisher {
            Some(p) => p,
            None => {
                warn!(
                    errors = cascade_errors.len(),
                    "DEAD_LETTER mode requested but no DLQ publisher configured"
                );
                return;
            }
        };

        for error in cascade_errors {
            let dead_letter = AngzarrDeadLetter::from_event_processing_failure(
                &crate::proto::EventBook {
                    cover: original_command.cover.clone(),
                    ..Default::default()
                },
                &error.error_message,
                0,     // No retries for CASCADE errors
                false, // Not transient
                &error.component_name,
                &error.component_type,
            )
            .with_metadata("source_domain", &error.source_domain)
            .with_metadata("cascade_error_mode", "DEAD_LETTER");

            if let Err(e) = dlq_publisher.publish(dead_letter).await {
                error!(
                    component = %error.component_name,
                    error = %e,
                    "Failed to publish cascade error to DLQ"
                );
            } else {
                info!(
                    component = %error.component_name,
                    source_domain = %error.source_domain,
                    "Cascade error published to DLQ"
                );
            }
        }
    }

    /// Execute CASCADE with COMPENSATE error mode.
    ///
    /// Tracks all executed commands. If any error occurs, compensates all
    /// previously executed commands in reverse order, then returns the error.
    #[tracing::instrument(name = "router.execute_with_compensate", skip_all, fields(domain = %command_book.domain()))]
    async fn execute_with_compensate(
        &self,
        command_book: CommandBook,
    ) -> Result<CommandResponse, Status> {
        let mut tracker = CascadeTracker::new();

        // Execute with CONTINUE mode internally to collect all results
        // We use a separate tracking method for COMPENSATE
        let result = self
            .execute_with_cascade_tracked(command_book.clone(), &mut tracker)
            .await;

        match result {
            Ok(response) => {
                // Check if any cascade errors occurred
                if response.cascade_errors.is_empty() {
                    Ok(response)
                } else {
                    // Errors occurred - compensate all executed commands
                    let first_error = response.cascade_errors.first().cloned();
                    self.compensate_commands(&tracker).await;

                    // Return the first error as the response
                    Err(Status::aborted(format!(
                        "CASCADE failed with compensation: {}",
                        first_error
                            .map(|e| e.error_message)
                            .unwrap_or_else(|| "unknown error".to_string())
                    )))
                }
            }
            Err(e) => {
                // Direct error - compensate all executed commands
                self.compensate_commands(&tracker).await;
                Err(e)
            }
        }
    }

    /// Compensate all tracked commands in reverse order.
    async fn compensate_commands(&self, tracker: &CascadeTracker) {
        for command in tracker.commands_for_compensation() {
            let domain = command.domain();
            debug!(domain = %domain, "Compensating command");

            if let Err(e) = self.execute_compensation(command.clone()).await {
                warn!(
                    domain = %domain,
                    error = %e,
                    "Compensation failed (continuing with remaining)"
                );
            }
        }
    }

    /// Execute CASCADE with command tracking for COMPENSATE mode.
    fn execute_with_cascade_tracked<'a>(
        &'a self,
        command_book: CommandBook,
        tracker: &'a mut CascadeTracker,
    ) -> futures::future::BoxFuture<'a, Result<CommandResponse, Status>> {
        use futures::FutureExt;
        let span =
            tracing::info_span!("router.execute_cascade_tracked", domain = %command_book.domain());

        async move {
            let (domain, _root_uuid) = parse_command_cover(&command_book)?;
            let (business, storage) = self.get_domain_resources(&domain)?;

            let ctx = self.create_context(storage, Some(crate::proto::SyncMode::Cascade));

            let mut response = execute_command_with_retry(
                &*ctx,
                &**business,
                command_book.clone(),
                saga_backoff(),
            )
            .await?;

            // Track this command as successfully executed
            tracker.record_success(command_book);

            // Call sync projectors
            if !self.sync_projectors.is_empty() {
                if let Some(ref events) = response.events {
                    let projections = self.call_sync_projectors(events).await;
                    response.projections.extend(projections);
                }
            }

            // Call sync sagas with tracking
            if !self.sync_sagas.is_empty() {
                if let Some(ref events) = response.events {
                    let saga_errors = self.call_sync_sagas_tracked(events, tracker).await?;
                    response.cascade_errors.extend(saga_errors);
                }
            }

            // Call sync PMs with tracking
            if !self.sync_pms.is_empty() {
                if let Some(ref events) = response.events {
                    let pm_errors = self.call_sync_pms_tracked(events, tracker).await?;
                    response.cascade_errors.extend(pm_errors);
                }
            }

            Ok(response)
        }
        .instrument(span)
        .boxed()
    }

    /// Execute a command with CASCADE mode with specified error handling mode.
    ///
    /// Internal version that accepts cascade_error_mode.
    #[tracing::instrument(name = "router.execute_cascade_internal", skip_all, fields(domain = %command_book.domain()))]
    async fn execute_with_cascade_internal(
        &self,
        command_book: CommandBook,
        cascade_error_mode: CascadeErrorMode,
    ) -> Result<CommandResponse, Status> {
        let (domain, _root_uuid) = parse_command_cover(&command_book)?;
        let (business, storage) = self.get_domain_resources(&domain)?;

        // CASCADE mode: set sync_mode on context so post_persist skips bus publishing
        let ctx = self.create_context(storage, Some(crate::proto::SyncMode::Cascade));

        let mut response =
            execute_command_with_retry(&*ctx, &**business, command_book, saga_backoff()).await?;

        // CASCADE: call sync projectors
        if !self.sync_projectors.is_empty() {
            if let Some(ref events) = response.events {
                let projections = self.call_sync_projectors(events).await;
                response.projections.extend(projections);
            }
        }

        // CASCADE: call sync sagas (recursive)
        if !self.sync_sagas.is_empty() {
            if let Some(ref events) = response.events {
                let saga_errors = self.call_sync_sagas(events, cascade_error_mode).await?;
                response.cascade_errors.extend(saga_errors);
            }
        }

        // CASCADE: call sync process managers
        if !self.sync_pms.is_empty() {
            if let Some(ref events) = response.events {
                let pm_errors = self.call_sync_pms(events, cascade_error_mode).await?;
                response.cascade_errors.extend(pm_errors);
            }
        }

        // CASCADE: do NOT publish to bus (events stay in-process)
        // Bus publishing happens only for non-CASCADE modes

        Ok(response)
    }

    /// Execute command with SIMPLE mode (sync projectors + bus publishing).
    ///
    /// This is the default execution mode. Projectors run synchronously and
    /// events are published to the bus for async processing by sagas/PMs.
    async fn execute_inner(&self, command_book: CommandBook) -> Result<CommandResponse, Status> {
        let (domain, root_uuid) = parse_command_cover(&command_book)?;

        debug!(
            domain = %domain,
            root = %root_uuid,
            "Executing command (SIMPLE mode)"
        );

        let (business, storage) = self.get_domain_resources(&domain)?;
        let ctx = self.create_context(storage, Some(crate::proto::SyncMode::Simple));

        let mut response =
            execute_command_with_retry(&*ctx, &**business, command_book, saga_backoff()).await?;

        // SIMPLE mode: call sync projectors
        if !self.sync_projectors.is_empty() {
            if let Some(ref events) = response.events {
                let projections = self.call_sync_projectors(events).await;
                response.projections.extend(projections);
            }
        }

        Ok(response)
    }

    /// Execute command with ASYNC mode (fire-and-forget).
    ///
    /// Persists events and publishes to bus, but does NOT call sync projectors.
    /// Fastest mode - returns immediately after persistence.
    pub async fn execute_async(
        &self,
        command_book: CommandBook,
    ) -> Result<CommandResponse, Status> {
        let (domain, root_uuid) = parse_command_cover(&command_book)?;

        debug!(
            domain = %domain,
            root = %root_uuid,
            "Executing command (ASYNC mode)"
        );

        let (business, storage) = self.get_domain_resources(&domain)?;
        let ctx = self.create_context(storage, Some(crate::proto::SyncMode::Async));

        // ASYNC mode: no sync projectors, just persist and publish
        execute_command_with_retry(&*ctx, &**business, command_book, saga_backoff()).await
    }

    /// Speculatively execute a command against temporal state (dry-run).
    ///
    /// Reconstructs aggregate state at a historical point in time, runs the
    /// handler, and returns the events that *would* be produced. This is purely
    /// speculative: no events are persisted to the store, no events are
    /// published to the bus, and no sagas or projectors are triggered. Use this
    /// to validate business rules or explore "what-if" scenarios without side
    /// effects.
    pub async fn speculative(
        &self,
        command_book: CommandBook,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) -> Result<CommandResponse, Status> {
        let (domain, root_uuid) = parse_command_cover(&command_book)?;

        debug!(
            domain = %domain,
            root = %root_uuid,
            ?as_of_sequence,
            ?as_of_timestamp,
            "Speculative command"
        );

        let (business, storage) = self.get_domain_resources(&domain)?;
        let ctx = self.create_context(storage, None);

        execute_command_pipeline(
            &*ctx,
            &**business,
            command_book,
            PipelineMode::Speculative {
                as_of_sequence,
                as_of_timestamp: as_of_timestamp.map(|s| s.to_string()),
            },
        )
        .await
    }

    /// Execute compensation for a rejected saga command.
    ///
    /// Returns the raw BusinessResponse so the caller can inspect revocation flags.
    /// If business logic returns events, they are persisted before returning.
    pub async fn execute_compensation(
        &self,
        command_book: CommandBook,
    ) -> Result<BusinessResponse, Status> {
        let (domain, root_uuid) = parse_command_cover(&command_book)?;
        let correlation_id = correlation::extract_correlation_id(&command_book)?;

        debug!(
            domain = %domain,
            root = %root_uuid,
            "Executing compensation"
        );

        let (business, storage) = self.get_domain_resources(&domain)?;
        let ctx = self.create_context(storage, None);

        let edition = command_book.edition().to_string();

        // Load prior events
        let prior_events = ctx
            .load_prior_events(&domain, &edition, root_uuid, &TemporalQuery::Current)
            .await?;

        // Transform events (upcasting)
        let prior_events = ctx.transform_events(&domain, prior_events).await?;

        // Invoke business logic
        let contextual_command = ContextualCommand {
            events: Some(prior_events.clone()),
            command: Some(command_book),
        };

        let response = business.invoke(contextual_command).await?;

        // If business returned events, persist them
        if let Some(business_response::Result::Events(ref events)) = response.result {
            if !events.pages.is_empty() {
                ctx.persist_events(
                    &prior_events,
                    events,
                    &domain,
                    &edition,
                    root_uuid,
                    &correlation_id,
                )
                .await?;

                // Post-persist: publish to bus
                ctx.post_persist(events).await?;
            }
        }

        Ok(response)
    }

    /// Inject fact events into an aggregate.
    ///
    /// Facts are external realities that cannot be rejected by business logic.
    /// The coordinator assigns sequence numbers and persists/publishes the events.
    ///
    /// Idempotent: TODO - subsequent requests with same external_id return original events.
    pub async fn inject_fact(
        &self,
        fact_events: crate::proto::EventBook,
        route_to_handler: bool,
    ) -> Result<crate::orchestration::aggregate::FactResponse, Status> {
        use crate::orchestration::aggregate::{execute_fact_pipeline, parse_event_cover};

        let (domain, _root_uuid) = parse_event_cover(&fact_events)?;
        let storage = self.get_storage(&domain)?;

        // Get client logic from router if available and routing is enabled
        let client_logic = if route_to_handler {
            self.get_client_logic(&domain)
        } else {
            None
        };

        let ctx = self.create_context(storage, None);

        // Execute fact pipeline
        execute_fact_pipeline(ctx.as_ref(), client_logic.as_deref(), fact_events).await
    }

    /// Get storage for a domain.
    #[allow(clippy::result_large_err)]
    pub fn get_storage(&self, domain: &str) -> Result<&DomainStorage, Status> {
        self.stores
            .get(domain)
            .ok_or_else(|| Status::not_found(format!("No storage configured for domain: {domain}")))
    }
}

#[async_trait::async_trait]
impl FactExecutor for CommandRouter {
    async fn inject(&self, fact: crate::proto::EventBook) -> Result<(), FactInjectionError> {
        // Extract domain before moving fact
        let domain = fact
            .cover
            .as_ref()
            .map(|c| c.domain.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // Use inject_fact with route_to_handler=true to allow aggregate's handle_fact
        // to transform the event if needed
        self.inject_fact(fact, true).await.map_err(|status| {
            if status.code() == tonic::Code::NotFound {
                FactInjectionError::AggregateNotFound { domain }
            } else {
                FactInjectionError::Internal(status.message().to_string())
            }
        })?;

        Ok(())
    }
}

#[cfg(test)]
#[path = "router.test.rs"]
mod tests;
