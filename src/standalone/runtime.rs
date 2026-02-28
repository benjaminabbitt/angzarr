//! Runtime implementation for standalone mode.
//!
//! Orchestrates storage, messaging, and handlers for local development.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::task::JoinHandle;
use tracing::info;

use crate::bus::{EventBus, MessagingConfig};
use crate::discovery::k8s::K8sServiceDiscovery;
use crate::discovery::ServiceDiscovery;
use crate::handlers::core::{
    AggregateCommandHandler, ProcessManagerEventHandler, ProjectorEventHandler, SagaEventHandler,
    SyncProjectorEntry as HandlerSyncProjectorEntry,
};
use crate::orchestration::aggregate::local::LocalAggregateContextFactory;
use crate::orchestration::aggregate::{AggregateContextFactory, ClientLogic};
use crate::orchestration::command::local::LocalCommandExecutor;
use crate::orchestration::destination::local::LocalDestinationFetcher;
use crate::orchestration::process_manager::local::LocalPMContextFactory;
use crate::orchestration::saga::local::LocalSagaContextFactory;
use crate::orchestration::FactExecutor;
use crate::proto::CommandBook;
use crate::proto_ext::CoverExt;
use crate::storage::{EventStore, SnapshotStore, StorageConfig};
use crate::transport::TransportConfig;

use super::client::CommandClient;
use super::dispatcher::CommandDispatcher;
use super::grpc_handlers::{CommandHandlerAdapter, ProcessManagerHandlerAdapter};
use super::router::{CommandRouter, DomainStorage, SyncProjectorEntry};
use super::server::ServerInfo;
use super::speculative::SpeculativeExecutor;
use super::traits::{
    CommandHandler, ProcessManagerConfig, ProcessManagerHandler, ProjectorConfig, ProjectorHandler,
    SagaConfig, SagaHandler,
};

/// Standalone runtime for angzarr.
///
/// Manages all components for running angzarr locally:
/// - Storage (events and snapshots per domain)
/// - Event bus (for pub/sub)
/// - Aggregate handlers (client logic)
/// - Projector handlers (read models)
/// - Saga handlers (cross-aggregate workflows)
pub struct Runtime {
    /// Per-domain storage.
    domain_stores: HashMap<String, DomainStorage>,
    /// Event bus for publishing and subscriber creation.
    event_bus: Arc<dyn EventBus>,
    /// Command router for dispatching commands to aggregates (legacy).
    router: Arc<CommandRouter>,
    /// Command dispatcher using per-domain handlers (new pattern).
    dispatcher: Arc<CommandDispatcher>,
    /// Speculative executor for dry-run of projectors, sagas, and PMs.
    speculative: Arc<SpeculativeExecutor>,
    /// Background task handles.
    tasks: Vec<JoinHandle<()>>,
    /// gRPC servers for cleanup on shutdown.
    servers: Vec<ServerInfo>,
}

/// Entry for a registered projector.
struct ProjectorEntry {
    name: String,
    handler: Arc<dyn ProjectorHandler>,
    config: ProjectorConfig,
}

impl Runtime {
    /// Create a new runtime (called by RuntimeBuilder).
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new(
        default_storage_config: StorageConfig,
        domain_storage_configs: HashMap<String, StorageConfig>,
        _messaging_config: MessagingConfig,
        _transport_config: TransportConfig,
        command_handlers: HashMap<String, Arc<dyn CommandHandler>>,
        projectors: HashMap<String, (Arc<dyn ProjectorHandler>, ProjectorConfig)>,
        sagas: HashMap<String, (Arc<dyn SagaHandler>, SagaConfig)>,
        process_managers: HashMap<String, (Arc<dyn ProcessManagerHandler>, ProcessManagerConfig)>,
        event_bus: Arc<dyn EventBus>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize per-domain storage
        let mut domain_stores = HashMap::new();

        for domain in command_handlers.keys() {
            // Use domain-specific config if available, otherwise fall back to default
            let storage_config = domain_storage_configs
                .get(domain)
                .unwrap_or(&default_storage_config);

            let (event_store, snapshot_store) =
                crate::storage::init_storage(storage_config).await?;

            info!(
                domain = %domain,
                storage_type = ?storage_config.storage_type,
                "Initialized storage for domain"
            );

            domain_stores.insert(
                domain.clone(),
                DomainStorage {
                    event_store,
                    snapshot_store,
                },
            );
        }

        // Initialize storage for process manager domains (PMs are aggregates)
        for (name, (_, config)) in &process_managers {
            if !domain_stores.contains_key(&config.domain) {
                let storage_config = domain_storage_configs
                    .get(&config.domain)
                    .unwrap_or(&default_storage_config);

                let (event_store, snapshot_store) =
                    crate::storage::init_storage(storage_config).await?;

                info!(
                    pm = %name,
                    domain = %config.domain,
                    storage_type = ?storage_config.storage_type,
                    "Initialized storage for process manager domain"
                );

                domain_stores.insert(
                    config.domain.clone(),
                    DomainStorage {
                        event_store,
                        snapshot_store,
                    },
                );
            }
        }

        info!(
            domains = command_handlers.len(),
            projectors = projectors.len(),
            sagas = sagas.len(),
            process_managers = process_managers.len(),
            "Runtime initialized"
        );

        // Wrap command handlers as ClientLogic (in-process, no TCP bridge)
        let mut business: HashMap<String, Arc<dyn ClientLogic>> = HashMap::new();
        for (domain, handler) in command_handlers {
            business.insert(domain, Arc::new(CommandHandlerAdapter::new(handler)));
        }

        // Register PM domains as command handlers (PMs are aggregates)
        // This allows Notification commands to route to PMs for compensation
        for (handler, config) in process_managers.values() {
            if !business.contains_key(&config.domain) {
                business.insert(
                    config.domain.clone(),
                    Arc::new(ProcessManagerHandlerAdapter::new(handler.clone())),
                );
            }
        }

        let servers = Vec::new();

        // Service discovery (unused for sync projectors — those are called in-process)
        let discovery: Arc<dyn ServiceDiscovery> = Arc::new(K8sServiceDiscovery::new_static());

        // Clone handler Arcs for speculative executor (before consumption into bus subscribers)
        // Include subscription info for domain-based routing
        let spec_projectors: HashMap<String, (Arc<dyn ProjectorHandler>, Vec<String>)> = projectors
            .iter()
            .map(|(name, (handler, config))| {
                (name.clone(), (handler.clone(), config.domains.clone()))
            })
            .collect();
        let spec_sagas: HashMap<String, (Arc<dyn SagaHandler>, String)> = sagas
            .iter()
            .map(|(name, (handler, config))| {
                (name.clone(), (handler.clone(), config.input_domain.clone()))
            })
            .collect();
        #[allow(clippy::type_complexity)]
        let spec_pms: HashMap<
            String,
            (
                Arc<dyn ProcessManagerHandler>,
                String,
                Vec<crate::descriptor::Target>,
            ),
        > = process_managers
            .iter()
            .map(|(name, (handler, config))| {
                (
                    name.clone(),
                    (
                        handler.clone(),
                        config.domain.clone(),
                        config.subscriptions.clone(),
                    ),
                )
            })
            .collect();

        // Convert projectors to entries
        let projector_entries: Vec<ProjectorEntry> = projectors
            .into_iter()
            .map(|(name, (handler, config))| ProjectorEntry {
                name,
                handler,
                config,
            })
            .collect();

        // Extract sync projector entries for the command router
        let sync_projector_entries: Vec<SyncProjectorEntry> = projector_entries
            .iter()
            .filter(|e| e.config.synchronous)
            .map(|e| SyncProjectorEntry {
                name: e.name.clone(),
                handler: e.handler.clone(),
            })
            .collect();

        // Create command router with in-process sync projectors (legacy).
        let router = Arc::new(CommandRouter::new(
            business.clone(),
            domain_stores.clone(),
            discovery.clone(),
            event_bus.clone(),
            sync_projector_entries.clone(),
            None,
        ));

        // Create per-domain handlers using factory pattern (new architecture).
        // This creates AggregateCommandHandler per domain, each with its own factory.
        let mut aggregate_handlers: HashMap<String, Arc<AggregateCommandHandler>> = HashMap::new();
        for (domain, client_logic) in &business {
            if let Some(storage) = domain_stores.get(domain) {
                // Create factory for this domain
                let factory = Arc::new(LocalAggregateContextFactory::new(
                    domain.clone(),
                    storage.clone(),
                    discovery.clone(),
                    event_bus.clone(),
                    client_logic.clone(),
                ));

                // Convert sync projector entries to handler format
                let handler_sync_projectors: Vec<HandlerSyncProjectorEntry> =
                    sync_projector_entries
                        .iter()
                        .map(|e| HandlerSyncProjectorEntry {
                            name: e.name.clone(),
                            handler: e.handler.clone(),
                        })
                        .collect();

                // Create handler with factory and sync projectors
                let handler = Arc::new(
                    AggregateCommandHandler::new(factory)
                        .with_sync_projectors(handler_sync_projectors),
                );

                aggregate_handlers.insert(domain.clone(), handler);
            }
        }

        // Create dispatcher with per-domain handlers
        let dispatcher = Arc::new(CommandDispatcher::new(aggregate_handlers));

        // Start event distribution for sagas, PMs, and async projectors
        let executor = Arc::new(LocalCommandExecutor::new(router.clone()));
        let fetcher = Arc::new(LocalDestinationFetcher::new(domain_stores.clone()));

        // Async projectors — each gets its own subscriber
        for entry in &projector_entries {
            let handler = ProjectorEventHandler::with_config(
                entry.handler.clone(),
                None,
                entry.config.domains.clone(),
                entry.config.synchronous,
                entry.name.clone(),
            );
            let sub = event_bus
                .create_subscriber(&format!("projector-{}", entry.name), None)
                .await?;
            sub.subscribe(Box::new(handler)).await?;
            sub.start_consuming().await?;
        }

        // Sagas — domain-filtered subscribers
        // Cast router to FactExecutor for fact injection support
        let fact_executor: Arc<dyn FactExecutor> = router.clone();
        for (name, (handler, config)) in sagas {
            let factory = Arc::new(LocalSagaContextFactory::new(handler, name.clone()));
            let validator = build_output_domain_validator(&name, &config.output_domains);
            let handler = SagaEventHandler::from_factory_with_validator(
                factory,
                executor.clone(),
                Some(fetcher.clone()),
                Some(fact_executor.clone()),
                Some(Arc::new(validator)),
                crate::utils::retry::saga_backoff(),
            );
            let sub = event_bus
                .create_subscriber(&format!("saga-{name}"), Some(&config.input_domain))
                .await?;
            sub.subscribe(Box::new(handler)).await?;
            sub.start_consuming().await?;
        }

        // Process managers — subscriber_all with handler-level subscription filtering
        for (name, (handler, config)) in process_managers {
            let subscriptions = config.subscriptions.clone();
            let pm_store = match domain_stores.get(&config.domain) {
                Some(store) => store.clone(),
                None => continue,
            };
            let factory = Arc::new(LocalPMContextFactory::new(
                handler,
                name.clone(),
                config.domain,
                pm_store,
                event_bus.clone(),
            ));
            let pm_handler = ProcessManagerEventHandler::from_factory(
                factory,
                fetcher.clone(),
                executor.clone(),
            )
            .with_fact_executor(Some(fact_executor.clone()))
            .with_targets(subscriptions);
            let sub = event_bus
                .create_subscriber(&format!("pm-{name}"), None)
                .await?;
            sub.subscribe(Box::new(pm_handler)).await?;
            sub.start_consuming().await?;
        }

        info!("Event distribution started");

        let speculative = Arc::new(SpeculativeExecutor::new(
            spec_projectors,
            spec_sagas,
            spec_pms,
            domain_stores.clone(),
        ));

        Ok(Self {
            domain_stores,
            event_bus,
            router,
            dispatcher,
            speculative,
            tasks: Vec::new(),
            servers,
        })
    }

    /// Get a command client for programmatic command submission.
    ///
    /// The client can be cloned and shared across tasks.
    /// Implements `GatewayClient` for unified API.
    pub fn command_client(&self) -> CommandClient {
        CommandClient::new(self.router.clone())
    }

    /// Get a query client for programmatic event retrieval.
    ///
    /// Routes queries by domain to the appropriate storage.
    /// Implements `QueryClient` for unified API.
    pub fn query_client(&self) -> super::client::StandaloneQueryClient {
        super::client::StandaloneQueryClient::new(self.domain_stores.clone())
    }

    /// Get storage for a specific domain.
    pub fn storage(&self, domain: &str) -> Option<&DomainStorage> {
        self.domain_stores.get(domain)
    }

    /// Get the event store for a specific domain.
    pub fn event_store(&self, domain: &str) -> Option<Arc<dyn EventStore>> {
        self.domain_stores
            .get(domain)
            .map(|s| s.event_store.clone())
    }

    /// Get the snapshot store for a specific domain.
    pub fn snapshot_store(&self, domain: &str) -> Option<Arc<dyn SnapshotStore>> {
        self.domain_stores
            .get(domain)
            .map(|s| s.snapshot_store.clone())
    }

    /// Get all domain stores.
    pub fn domain_stores(&self) -> &HashMap<String, DomainStorage> {
        &self.domain_stores
    }

    /// Get access to the event bus (for publishing).
    pub fn event_bus(&self) -> Arc<dyn EventBus> {
        self.event_bus.clone()
    }

    /// Get the command router (legacy).
    pub fn router(&self) -> Arc<CommandRouter> {
        self.router.clone()
    }

    /// Get the command dispatcher (new per-domain handler architecture).
    pub fn dispatcher(&self) -> Arc<CommandDispatcher> {
        self.dispatcher.clone()
    }

    /// Get the speculative executor for dry-run of projectors, sagas, and PMs.
    ///
    /// The executor holds `Arc` clones of the same handler instances registered
    /// with this runtime. Speculative execution invokes the same client logic
    /// without persistence, publishing, or command execution.
    pub fn speculative_executor(&self) -> Arc<SpeculativeExecutor> {
        self.speculative.clone()
    }

    /// Get a speculative client for dry-run of commands, projectors, sagas, and PMs.
    ///
    /// Convenience wrapper around `speculative_executor()`.
    pub fn speculative_client(&self) -> super::client::SpeculativeClient {
        super::client::SpeculativeClient::new(self.speculative.clone(), self.router.clone())
    }

    /// Start the runtime without blocking.
    ///
    /// This starts event distribution to projectors and sagas.
    /// Use this for testing or when you need to interact with the runtime
    /// programmatically after starting.
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Runtime started");
        Ok(())
    }

    /// Inject fact events into an aggregate.
    ///
    /// Fact events represent external realities that cannot be rejected. The runtime:
    /// 1. Validates the Cover and extracts domain/root
    /// 2. Routes to the aggregate's fact handler (if registered)
    /// 3. Assigns real sequence numbers (replacing FactSequence markers)
    /// 4. Persists and publishes the events
    ///
    /// # Arguments
    ///
    /// * `fact_events` - EventBook containing fact events with FactSequence markers.
    ///   Must have `Cover.external_id` set for idempotency.
    ///
    /// # Returns
    ///
    /// The persisted events with real sequence numbers.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let fact = EventBook {
    ///     cover: Some(Cover {
    ///         domain: "payments".into(),
    ///         root: Some(order_id.into()),
    ///         external_id: "stripe_pi_abc123".into(),
    ///         ..Default::default()
    ///     }),
    ///     pages: vec![EventPage {
    ///         sequence_type: Some(SequenceType::Fact(FactSequence {
    ///             source: "stripe".into(),
    ///             description: "Payment confirmed".into(),
    ///         })),
    ///         payload: Some(Payload::Event(payment_received)),
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// };
    ///
    /// let result = runtime.inject_fact(fact).await?;
    /// ```
    pub async fn inject_fact(
        &self,
        fact_events: crate::proto::EventBook,
    ) -> Result<crate::orchestration::aggregate::FactResponse, tonic::Status> {
        use crate::orchestration::aggregate::{execute_fact_pipeline, parse_event_cover};

        let (domain, _root_uuid) = parse_event_cover(&fact_events)?;

        // Get storage for this domain
        let storage = self.domain_stores.get(&domain).ok_or_else(|| {
            tonic::Status::not_found(format!("No storage registered for domain '{}'", domain))
        })?;

        // Create a local aggregate context factory for this domain
        let discovery: Arc<dyn ServiceDiscovery> = Arc::new(K8sServiceDiscovery::new_static());

        // Get client logic from router if available (for routing facts to aggregate)
        let client_logic = self.router.get_client_logic(&domain);

        // Create context using LocalAggregateContextFactory
        let factory = crate::orchestration::aggregate::local::LocalAggregateContextFactory::new(
            domain.clone(),
            storage.clone(),
            discovery,
            self.event_bus.clone(),
            client_logic.clone().unwrap_or_else(|| {
                Arc::new(super::grpc_handlers::NoOpClientLogic) as Arc<dyn ClientLogic>
            }),
        );
        let ctx = factory.create();

        // Execute fact pipeline
        execute_fact_pipeline(ctx.as_ref(), client_logic.as_deref(), fact_events).await
    }

    /// Run the runtime until Ctrl+C.
    ///
    /// This starts all background tasks and waits for shutdown signal.
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Runtime running, press Ctrl+C to exit");

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await?;

        info!("Shutting down runtime");

        // Cancel all background tasks
        for task in self.tasks {
            task.abort();
        }

        // Shutdown gRPC servers
        for server in self.servers {
            server.shutdown();
        }

        // Flush OTel buffers before exit
        crate::utils::bootstrap::shutdown_telemetry();

        Ok(())
    }
}

/// Build output domain validator for a saga.
fn build_output_domain_validator(
    saga_name: &str,
    output_domains: &[String],
) -> impl Fn(&CommandBook) -> Result<(), String> + Send + Sync {
    let name = saga_name.to_string();
    let domains = output_domains.to_vec();
    move |cmd: &CommandBook| -> Result<(), String> {
        let target = cmd.domain();
        if domains.iter().any(|d| d == target) {
            Ok(())
        } else {
            Err(format!(
                "saga '{}': command targets domain '{}' but configured output_domains are {:?}",
                name, target, domains
            ))
        }
    }
}
