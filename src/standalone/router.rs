//! Command routing for standalone runtime.
//!
//! Dispatches commands to registered aggregate client logic.

use std::collections::HashMap;
use std::sync::Arc;

use tonic::Status;
use tracing::{debug, info, warn, Instrument};
use uuid::Uuid;

use crate::bus::EventBus;
use crate::discovery::ServiceDiscovery;
use crate::orchestration::aggregate::local::LocalAggregateContext;
use crate::orchestration::aggregate::TemporalQuery;
use crate::orchestration::aggregate::{
    execute_command_pipeline, execute_command_with_retry, parse_command_cover, AggregateContext,
    ClientLogic, PipelineMode,
};
use crate::orchestration::correlation;
use crate::orchestration::{FactExecutor, FactInjectionError};
use crate::proto::{
    business_response, BusinessResponse, CommandBook, CommandResponse, ContextualCommand,
};
use crate::proto_ext::CoverExt;
use crate::storage::{EventStore, SnapshotStore};
use crate::utils::retry::saga_backoff;

use super::traits::{ProjectorHandler, SagaHandler};

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
    /// The name of the edition this router is operating within, if any.
    edition_name: Option<String>,
}

impl CommandRouter {
    /// Create a new command router.
    pub fn new(
        business: HashMap<String, Arc<dyn ClientLogic>>,
        stores: HashMap<String, DomainStorage>,
        discovery: Arc<dyn ServiceDiscovery>,
        event_bus: Arc<dyn EventBus>,
        sync_projectors: Vec<SyncProjectorEntry>,
        sync_sagas: Vec<SyncSagaEntry>,
        edition_name: Option<String>,
    ) -> Self {
        let domains: Vec<_> = business.keys().cloned().collect();
        info!(
            domains = ?domains,
            sync_projectors = sync_projectors.len(),
            sync_sagas = sync_sagas.len(),
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
            edition_name,
        }
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

        Arc::new(match sync_mode {
            Some(mode) => ctx.with_sync_mode(mode),
            None => ctx,
        })
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
    /// Returns a BoxFuture to support async recursion (sagas trigger commands
    /// which trigger more sagas).
    fn call_sync_sagas<'a>(
        &'a self,
        events: &'a crate::proto::EventBook,
    ) -> futures::future::BoxFuture<'a, Result<(), Status>> {
        use futures::FutureExt;
        let source_domain = events.domain().to_string();
        let span = tracing::info_span!("router.sync_sagas", %source_domain);

        async move {
            use crate::proto_ext::CoverExt;

            let source_domain = events.domain();

            // Skip infrastructure domains
            if source_domain.starts_with('_') {
                return Ok(());
            }

            // Find sagas subscribed to this domain
            let matching_sagas: Vec<_> = self
                .sync_sagas
                .iter()
                .filter(|s| s.source_domain == source_domain)
                .collect();

            if matching_sagas.is_empty() {
                return Ok(());
            }

            let edition = events.edition().to_string();

            for entry in matching_sagas {
                // Phase 1: Prepare - get destination covers
                let mut covers = match entry.handler.prepare(events).await {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(
                            saga = %entry.name,
                            error = %e,
                            "Sync saga prepare failed"
                        );
                        continue;
                    }
                };

                // Stamp edition on covers
                for cover in &mut covers {
                    cover.stamp_edition_if_empty(&edition);
                }

                // Fetch destination state
                let mut destinations = Vec::with_capacity(covers.len());
                for cover in &covers {
                    let dest_domain = &cover.domain;
                    let dest_root = cover
                        .root
                        .as_ref()
                        .map(|r| Uuid::from_slice(&r.value).unwrap_or_default())
                        .unwrap_or_default();

                    if let Some(storage) = self.stores.get(dest_domain) {
                        match storage
                            .event_store
                            .get(dest_domain, &edition, dest_root)
                            .await
                        {
                            Ok(pages) => {
                                let mut book = crate::proto::EventBook {
                                    cover: Some(cover.clone()),
                                    pages,
                                    ..Default::default()
                                };
                                crate::proto_ext::calculate_set_next_seq(&mut book);
                                destinations.push(book);
                            }
                            Err(e) => {
                                warn!(
                                    saga = %entry.name,
                                    domain = %dest_domain,
                                    error = %e,
                                    "Failed to fetch destination state"
                                );
                            }
                        }
                    }
                }

                // Phase 2: Handle - get commands
                let mut response = match entry.handler.handle(events, &destinations).await {
                    Ok(r) => r,
                    Err(e) => {
                        warn!(
                            saga = %entry.name,
                            error = %e,
                            "Sync saga handle failed"
                        );
                        continue;
                    }
                };

                // Stamp edition on commands
                for cmd in &mut response.commands {
                    if let Some(c) = &mut cmd.cover {
                        c.stamp_edition_if_empty(&edition);
                    }
                }

                debug!(
                    saga = %entry.name,
                    commands = response.commands.len(),
                    "Sync saga produced commands"
                );

                // Recursively execute commands with CASCADE mode
                for command in response.commands {
                    match self.execute_with_cascade(command).await {
                        Ok(_) => {}
                        Err(e) => {
                            warn!(
                                saga = %entry.name,
                                error = %e,
                                "Sync saga command execution failed"
                            );
                        }
                    }
                }
            }

            Ok(())
        }
        .instrument(span)
        .boxed()
    }

    /// Execute a command with CASCADE mode (sync projectors + sync sagas).
    ///
    /// Used by sync sagas to recursively execute their output commands.
    /// Also used by `LocalCommandExecutor` when receiving a CASCADE command.
    #[tracing::instrument(name = "router.execute_cascade", skip_all, fields(domain = %command_book.domain()))]
    pub async fn execute_with_cascade(
        &self,
        command_book: CommandBook,
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
                self.call_sync_sagas(events).await?;
            }
        }

        // CASCADE: do NOT publish to bus (events stay in-process)
        // Bus publishing happens only for non-CASCADE modes

        Ok(response)
    }

    /// Core command execution with sequence validation.
    async fn execute_inner(&self, command_book: CommandBook) -> Result<CommandResponse, Status> {
        let (domain, root_uuid) = parse_command_cover(&command_book)?;

        debug!(
            domain = %domain,
            root = %root_uuid,
            "Executing command"
        );

        let (business, storage) = self.get_domain_resources(&domain)?;
        let ctx = self.create_context(storage, None);

        let mut response =
            execute_command_with_retry(&*ctx, &**business, command_book, saga_backoff()).await?;

        // Call in-process sync projectors (standalone mode)
        if !self.sync_projectors.is_empty() {
            if let Some(ref events) = response.events {
                let projections = self.call_sync_projectors(events).await;
                response.projections.extend(projections);
            }
        }

        Ok(response)
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
use crate::proto::{Cover, MergeStrategy, Uuid as ProtoUuid};

/// Helper to create a command book for tests.
#[cfg(test)]
pub fn create_command_book(
    domain: &str,
    root: Uuid,
    command_type: &str,
    command_data: Vec<u8>,
) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
            external_id: String::new(),
        }),
        pages: vec![crate::proto::CommandPage {
            sequence: 0,
            payload: Some(crate::proto::command_page::Payload::Command(
                prost_types::Any {
                    type_url: command_type.to_string(),
                    value: command_data,
                },
            )),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
        }],
        saga_origin: None,
    }
}

#[cfg(test)]
#[path = "router.test.rs"]
mod tests;
