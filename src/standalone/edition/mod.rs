//! Edition support — persistent divergence of event timelines.
//!
//! An edition shares main timeline history up to a divergence point,
//! then continues independently with its own events. No data is copied;
//! `EditionEventStore` composites main events (from `"angzarr.{domain}"`)
//! up to divergence with edition-specific events (under `"{name}.{domain}"`).
//!
//! Business logic handlers are the **same `Arc` instances** used by the
//! main runtime — editions reuse registered handlers with edition-aware
//! storage and bus subscriptions.
//!
//! **Merging divergent editions back to the main timeline is architecturally
//! infeasible and will never be supported.** Editions are one-way forks.

pub mod aggregate_context;
pub mod event_store;
pub mod metadata;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tonic::Status;
use tracing::{info, warn};

use crate::bus::EventBus;
use crate::handlers::core::{ProcessManagerEventHandler, ProjectorEventHandler, SagaEventHandler};
use crate::orchestration::aggregate::BusinessLogic;
use crate::orchestration::command::local::LocalCommandExecutor;
use crate::orchestration::destination::local::LocalDestinationFetcher;
use crate::orchestration::process_manager::local::LocalPMContextFactory;
use crate::orchestration::projector::local::LocalProjectorContext;
use crate::orchestration::saga::local::LocalSagaContextFactory;
use crate::proto::{CommandBook, CommandResponse};

use super::grpc_handlers::AggregateHandlerAdapter;
use super::router::{CommandRouter, DomainStorage};
use super::traits::{
    AggregateHandler, ProcessManagerConfig, ProcessManagerHandler, ProjectorConfig,
    ProjectorHandler, SagaConfig, SagaHandler,
};

pub use aggregate_context::EditionAggregateContext;
pub use event_store::EditionEventStore;
pub use metadata::{DivergencePoint, EditionMetadata};

/// References to all handler types for creating edition runtimes.
pub struct EditionHandlerRefs {
    pub aggregates: HashMap<String, Arc<dyn AggregateHandler>>,
    pub projectors: HashMap<String, (Arc<dyn ProjectorHandler>, ProjectorConfig)>,
    pub sagas: HashMap<String, (Arc<dyn SagaHandler>, SagaConfig)>,
    pub process_managers: HashMap<String, (Arc<dyn ProcessManagerHandler>, ProcessManagerConfig)>,
}

/// A running edition with its own storage and bus subscriptions.
struct EditionRuntime {
    metadata: EditionMetadata,
    router: Arc<CommandRouter>,
    domain_stores: HashMap<String, DomainStorage>,
}

/// Manages edition lifecycle: create, delete, list, and command routing.
pub struct EditionManager {
    editions: Arc<RwLock<HashMap<String, EditionRuntime>>>,
    handlers: Arc<EditionHandlerRefs>,
    base_stores: HashMap<String, DomainStorage>,
    event_bus: Arc<dyn EventBus>,
}

impl EditionManager {
    /// Create a new edition manager.
    pub fn new(
        handlers: EditionHandlerRefs,
        base_stores: HashMap<String, DomainStorage>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            editions: Arc::new(RwLock::new(HashMap::new())),
            handlers: Arc::new(handlers),
            base_stores,
            event_bus,
        }
    }

    /// Create a new edition at the given divergence point.
    pub async fn create_edition(
        &self,
        name: String,
        divergence: DivergencePoint,
        description: String,
    ) -> Result<EditionMetadata, Status> {
        let editions = self.editions.read().await;
        if editions.contains_key(&name) {
            return Err(Status::already_exists(format!(
                "Edition already exists: {name}"
            )));
        }
        drop(editions);

        let metadata = EditionMetadata {
            name: name.clone(),
            divergence: divergence.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            description,
        };

        // Build edition-aware storage for each aggregate domain
        let mut domain_stores = HashMap::new();
        for (domain, base_storage) in &self.base_stores {
            let edition_store = Arc::new(EditionEventStore::new(
                base_storage.event_store.clone(),
                name.clone(),
                divergence.clone(),
            ));
            domain_stores.insert(
                domain.clone(),
                DomainStorage {
                    event_store: edition_store,
                    snapshot_store: base_storage.snapshot_store.clone(),
                },
            );
        }

        // Build business logic adapters (same handlers)
        let mut business: HashMap<String, Arc<dyn BusinessLogic>> = HashMap::new();
        for (domain, handler) in &self.handlers.aggregates {
            business.insert(
                domain.clone(),
                Arc::new(AggregateHandlerAdapter::new(handler.clone())),
            );
        }

        // Build edition command router
        let discovery: Arc<dyn crate::discovery::ServiceDiscovery> =
            Arc::new(crate::discovery::k8s::K8sServiceDiscovery::new_static());
        let router = Arc::new(CommandRouter::new(
            business,
            domain_stores.clone(),
            discovery,
            self.event_bus.clone(),
            vec![],
        ));

        // Set up edition subscribers on the channel bus
        let executor = Arc::new(LocalCommandExecutor::new(router.clone()));
        let fetcher = Arc::new(LocalDestinationFetcher::new(domain_stores.clone()));

        // Projectors — subscribe to edition-prefixed domains
        for (proj_name, (handler, config)) in &self.handlers.projectors {
            let ctx = Arc::new(LocalProjectorContext::new(handler.clone()));
            let edition_domains: Vec<String> = config
                .domains
                .iter()
                .map(|d| format!("{}.{}", name, d))
                .collect();
            let handler = ProjectorEventHandler::with_config(
                ctx,
                None,
                edition_domains,
                config.synchronous,
                format!("{}.{}", name, proj_name),
            );
            let sub_name = format!("edition-{}-projector-{}", name, proj_name);
            match self.event_bus.create_subscriber(&sub_name, None).await {
                Ok(sub) => {
                    if let Err(e) = sub.subscribe(Box::new(handler)).await {
                        warn!(
                            edition = %name,
                            projector = %proj_name,
                            error = %e,
                            "Failed to subscribe edition projector"
                        );
                    }
                    if let Err(e) = sub.start_consuming().await {
                        warn!(
                            edition = %name,
                            projector = %proj_name,
                            error = %e,
                            "Failed to start edition projector consumer"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        edition = %name,
                        projector = %proj_name,
                        error = %e,
                        "Failed to create edition projector subscriber"
                    );
                }
            }
        }

        // Sagas — subscribe to edition-prefixed input domain
        for (saga_name, (handler, config)) in &self.handlers.sagas {
            let factory = Arc::new(LocalSagaContextFactory::new(
                handler.clone(),
                format!("{}.{}", name, saga_name),
            ));
            let edition_input = format!("{}.{}", name, config.input_domain);
            let handler = SagaEventHandler::from_factory_with_validator(
                factory,
                executor.clone(),
                Some(fetcher.clone()),
                None,
                crate::utils::retry::saga_backoff(),
            );
            let sub_name = format!("edition-{}-saga-{}", name, saga_name);
            match self
                .event_bus
                .create_subscriber(&sub_name, Some(&edition_input))
                .await
            {
                Ok(sub) => {
                    if let Err(e) = sub.subscribe(Box::new(handler)).await {
                        warn!(
                            edition = %name,
                            saga = %saga_name,
                            error = %e,
                            "Failed to subscribe edition saga"
                        );
                    }
                    if let Err(e) = sub.start_consuming().await {
                        warn!(
                            edition = %name,
                            saga = %saga_name,
                            error = %e,
                            "Failed to start edition saga consumer"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        edition = %name,
                        saga = %saga_name,
                        error = %e,
                        "Failed to create edition saga subscriber"
                    );
                }
            }
        }

        // Process managers — subscribe to edition-prefixed domains
        for (pm_name, (handler, config)) in &self.handlers.process_managers {
            let subscriptions = handler.subscriptions();
            let pm_store = match domain_stores.get(&config.domain) {
                Some(store) => store.clone(),
                None => {
                    warn!(
                        edition = %name,
                        pm = %pm_name,
                        domain = %config.domain,
                        "No storage for PM domain, skipping"
                    );
                    continue;
                }
            };
            let factory = Arc::new(LocalPMContextFactory::new(
                handler.clone(),
                format!("{}.{}", name, pm_name),
                format!("{}.{}", name, config.domain),
                pm_store,
                self.event_bus.clone(),
            ));
            let pm_handler = ProcessManagerEventHandler::from_factory(
                factory,
                fetcher.clone(),
                executor.clone(),
            )
            .with_subscriptions(subscriptions);
            let sub_name = format!("edition-{}-pm-{}", name, pm_name);
            match self.event_bus.create_subscriber(&sub_name, None).await {
                Ok(sub) => {
                    if let Err(e) = sub.subscribe(Box::new(pm_handler)).await {
                        warn!(
                            edition = %name,
                            pm = %pm_name,
                            error = %e,
                            "Failed to subscribe edition PM"
                        );
                    }
                    if let Err(e) = sub.start_consuming().await {
                        warn!(
                            edition = %name,
                            pm = %pm_name,
                            error = %e,
                            "Failed to start edition PM consumer"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        edition = %name,
                        pm = %pm_name,
                        error = %e,
                        "Failed to create edition PM subscriber"
                    );
                }
            }
        }

        info!(edition = %name, "Edition created");

        let runtime = EditionRuntime {
            metadata: metadata.clone(),
            router,
            domain_stores,
        };

        self.editions.write().await.insert(name, runtime);
        Ok(metadata)
    }

    /// Delete an edition, removing its runtime and metadata.
    pub async fn delete_edition(&self, name: &str) -> Result<(), Status> {
        let mut editions = self.editions.write().await;
        if editions.remove(name).is_none() {
            return Err(Status::not_found(format!("Edition not found: {name}")));
        }
        info!(edition = %name, "Edition deleted");
        Ok(())
    }

    /// List all active editions.
    pub async fn list_editions(&self) -> Vec<EditionMetadata> {
        let editions = self.editions.read().await;
        editions.values().map(|r| r.metadata.clone()).collect()
    }

    /// Execute a command on an edition.
    pub async fn execute(
        &self,
        edition_name: &str,
        command: CommandBook,
    ) -> Result<CommandResponse, Status> {
        let editions = self.editions.read().await;
        let runtime = editions.get(edition_name).ok_or_else(|| {
            Status::not_found(format!("Edition not found: {edition_name}"))
        })?;
        runtime.router.execute(command).await
    }

    /// Get domain stores for an edition (for queries).
    pub async fn get_stores(
        &self,
        edition_name: &str,
    ) -> Option<HashMap<String, DomainStorage>> {
        let editions = self.editions.read().await;
        editions
            .get(edition_name)
            .map(|r| r.domain_stores.clone())
    }
}
