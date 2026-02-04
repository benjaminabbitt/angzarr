//! Runtime implementation for standalone mode.
//!
//! Orchestrates storage, messaging, and handlers for local development.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::bus::{EventBus, MessagingConfig};
use crate::discovery::k8s::K8sServiceDiscovery;
use crate::discovery::ServiceDiscovery;
use crate::handlers::core::{ProcessManagerEventHandler, ProjectorEventHandler, SagaEventHandler};
use crate::orchestration::aggregate::ClientLogic;
use crate::orchestration::command::local::LocalCommandExecutor;
use crate::orchestration::destination::local::LocalDestinationFetcher;
use crate::orchestration::process_manager::local::LocalPMContextFactory;
use crate::orchestration::saga::local::LocalSagaContextFactory;
use crate::proto::{CommandBook, ComponentDescriptor, Subscription};
use crate::proto_ext::CoverExt;
use crate::storage::{EventStore, SnapshotStore, StorageConfig};
use crate::transport::TransportConfig;

use super::builder::GatewayConfig;
use super::client::CommandClient;
use super::grpc_handlers::AggregateHandlerAdapter;
use super::router::{CommandRouter, DomainStorage, SyncProjectorEntry};
use super::server::ServerInfo;
use super::speculative::SpeculativeExecutor;
use super::traits::{
    AggregateHandler, ProcessManagerConfig, ProcessManagerHandler, ProjectorConfig,
    ProjectorHandler, SagaConfig, SagaHandler,
};

/// Standalone runtime for angzarr.
///
/// Manages all components for running angzarr locally:
/// - Storage (events and snapshots per domain)
/// - Event bus (for pub/sub)
/// - Aggregate handlers (client logic)
/// - Projector handlers (read models)
/// - Saga handlers (cross-aggregate workflows)
/// - Optional gateway (for external clients)
pub struct Runtime {
    /// Per-domain storage.
    domain_stores: HashMap<String, DomainStorage>,
    /// Event bus for publishing and subscriber creation.
    event_bus: Arc<dyn EventBus>,
    /// Command router for dispatching commands to aggregates.
    router: Arc<CommandRouter>,
    /// Speculative executor for dry-run of projectors, sagas, and PMs.
    speculative: Arc<SpeculativeExecutor>,
    /// Component descriptors collected from all registered handlers at build time.
    descriptors: Vec<ComponentDescriptor>,
    /// Background task handles.
    tasks: Vec<JoinHandle<()>>,
    /// gRPC servers for cleanup on shutdown.
    servers: Vec<ServerInfo>,
    /// Gateway configuration.
    gateway_config: GatewayConfig,
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
        gateway_config: GatewayConfig,
        aggregates: HashMap<String, Arc<dyn AggregateHandler>>,
        projectors: HashMap<String, (Arc<dyn ProjectorHandler>, ProjectorConfig)>,
        sagas: HashMap<String, (Arc<dyn SagaHandler>, SagaConfig)>,
        process_managers: HashMap<String, (Arc<dyn ProcessManagerHandler>, ProcessManagerConfig)>,
        event_bus: Arc<dyn EventBus>,
        #[cfg(feature = "topology")]
        topology_projector: Option<Arc<crate::handlers::projectors::topology::TopologyProjector>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Collect descriptors from all registered handlers before they're consumed.
        // Fills in name/component_type from registration keys and config when the
        // handler returns a default descriptor (pre-router migration).
        let descriptors = Self::collect_descriptors(
            &aggregates,
            &projectors,
            &sagas,
            &process_managers,
        );

        // Register component descriptors with topology projector for correct
        // node types (saga, projector, PM) in the topology graph.
        #[cfg(feature = "topology")]
        if let Some(ref topology) = topology_projector {
            topology.init().await.map_err(|e| {
                Box::<dyn std::error::Error>::from(format!("topology init failed: {e}"))
            })?;
            topology.register_components(&descriptors).await.map_err(|e| {
                Box::<dyn std::error::Error>::from(format!("topology registration failed: {e}"))
            })?;
            info!(components = descriptors.len(), "Topology components registered");
        }

        // Initialize per-domain storage
        let mut domain_stores = HashMap::new();

        for domain in aggregates.keys() {
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
            domains = aggregates.len(),
            projectors = projectors.len(),
            sagas = sagas.len(),
            process_managers = process_managers.len(),
            "Runtime initialized"
        );

        // Wrap aggregate handlers as ClientLogic (in-process, no TCP bridge)
        let mut business: HashMap<String, Arc<dyn ClientLogic>> = HashMap::new();
        for (domain, handler) in aggregates {
            business.insert(domain, Arc::new(AggregateHandlerAdapter::new(handler)));
        }

        let servers = Vec::new();

        // Service discovery (unused for sync projectors — those are called in-process)
        let discovery: Arc<dyn ServiceDiscovery> = Arc::new(K8sServiceDiscovery::new_static());

        // Clone handler Arcs for speculative executor (before consumption into bus subscribers)
        let spec_projectors: HashMap<String, Arc<dyn ProjectorHandler>> = projectors
            .iter()
            .map(|(name, (handler, _))| (name.clone(), handler.clone()))
            .collect();
        let spec_sagas: HashMap<String, Arc<dyn SagaHandler>> = sagas
            .iter()
            .map(|(name, (handler, _))| (name.clone(), handler.clone()))
            .collect();
        let spec_pms: HashMap<String, (Arc<dyn ProcessManagerHandler>, String)> = process_managers
            .iter()
            .map(|(name, (handler, config))| {
                (name.clone(), (handler.clone(), config.domain.clone()))
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

        // Create command router with in-process sync projectors.
        let router = Arc::new(CommandRouter::new(
            business,
            domain_stores.clone(),
            discovery.clone(),
            event_bus.clone(),
            sync_projector_entries,
            None,
        ));

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
        for (name, (handler, config)) in sagas {
            let factory = Arc::new(LocalSagaContextFactory::new(
                handler,
                name.clone(),
            ));
            let validator = build_output_domain_validator(&name, &config.output_domains);
            let handler = SagaEventHandler::from_factory_with_validator(
                factory,
                executor.clone(),
                Some(fetcher.clone()),
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
            let subscriptions = handler.descriptor().inputs;
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
            .with_subscriptions(subscriptions);
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
            speculative,
            descriptors,
            tasks: Vec::new(),
            servers,
            gateway_config,
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

    /// Get the command router.
    pub fn router(&self) -> Arc<CommandRouter> {
        self.router.clone()
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

    /// Get the component descriptors collected at build time.
    ///
    /// These describe all registered handlers (aggregates, projectors, sagas,
    /// process managers) with their names, types, and input subscriptions.
    /// Pass to `TopologyProjector::register_components()` for graph construction.
    pub fn descriptors(&self) -> &[ComponentDescriptor] {
        &self.descriptors
    }

    /// Collect descriptors from all registered handlers.
    ///
    /// Merges handler-declared descriptors with registration metadata (keys + config).
    /// Handlers returning default descriptors get name/type/inputs filled from config.
    fn collect_descriptors(
        aggregates: &HashMap<String, Arc<dyn AggregateHandler>>,
        projectors: &HashMap<String, (Arc<dyn ProjectorHandler>, ProjectorConfig)>,
        sagas: &HashMap<String, (Arc<dyn SagaHandler>, SagaConfig)>,
        process_managers: &HashMap<String, (Arc<dyn ProcessManagerHandler>, ProcessManagerConfig)>,
    ) -> Vec<ComponentDescriptor> {
        let mut descriptors = Vec::new();

        for (domain, handler) in aggregates {
            let mut desc = handler.descriptor();
            if desc.name.is_empty() {
                desc.name = domain.clone();
            }
            if desc.component_type.is_empty() {
                desc.component_type = "aggregate".to_string();
            }
            descriptors.push(desc);
        }

        for (name, (handler, config)) in projectors {
            let mut desc = handler.descriptor();
            if desc.name.is_empty() {
                desc.name = name.clone();
            }
            if desc.component_type.is_empty() {
                desc.component_type = "projector".to_string();
            }
            if desc.inputs.is_empty() && !config.domains.is_empty() {
                desc.inputs = config
                    .domains
                    .iter()
                    .map(|d| Subscription {
                        domain: d.clone(),
                        event_types: vec![],
                    })
                    .collect();
            }
            descriptors.push(desc);
        }

        for (name, (handler, config)) in sagas {
            let mut desc = handler.descriptor();
            if desc.name.is_empty() {
                desc.name = name.clone();
            }
            if desc.component_type.is_empty() {
                desc.component_type = "saga".to_string();
            }
            if desc.inputs.is_empty() {
                desc.inputs = vec![Subscription {
                    domain: config.input_domain.clone(),
                    event_types: vec![],
                }];
            }
            descriptors.push(desc);
        }

        for (name, (handler, _config)) in process_managers {
            let mut desc = handler.descriptor();
            if desc.name.is_empty() {
                desc.name = name.clone();
            }
            if desc.component_type.is_empty() {
                desc.component_type = "process_manager".to_string();
            }
            descriptors.push(desc);
        }

        descriptors
    }

    /// Start the runtime without blocking.
    ///
    /// This starts event distribution to projectors and sagas.
    /// Use this for testing or when you need to interact with the runtime
    /// programmatically after starting.
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Publish descriptors to event bus for topology discovery.
        // In standalone mode this reaches in-process topology projectors.
        // In distributed mode each binary publishes its own descriptors.
        if let Err(e) = crate::proto_ext::publish_descriptors(
            self.event_bus.as_ref(),
            &self.descriptors,
        )
        .await
        {
            warn!(error = %e, "Failed to publish component descriptors to event bus");
        }

        info!("Runtime started");
        Ok(())
    }

    /// Run the runtime until Ctrl+C.
    ///
    /// This starts all background tasks and waits for shutdown signal.
    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Start gateway if configured
        match &self.gateway_config {
            GatewayConfig::None => {
                info!("No gateway configured, running in standalone-only mode");
            }
            GatewayConfig::Tcp(port) => {
                info!(port = %port, "Starting TCP gateway");
                self.start_gateway().await?;
            }
            GatewayConfig::Uds(path) => {
                info!(path = %path.display(), "Starting UDS gateway");
                self.start_gateway().await?;
            }
        }

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

        Ok(())
    }

    /// Start the gateway server.
    ///
    /// Serves `CommandGateway` and `EventQuery` directly using the standalone
    /// router and domain stores. No bridge server or service discovery.
    async fn start_gateway(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use super::server::{StandaloneEventQueryBridge, StandaloneGatewayService};
        use crate::proto::command_gateway_server::CommandGatewayServer;
        use crate::proto::event_query_server::EventQueryServer;

        let gateway = StandaloneGatewayService::new(self.router.clone());
        let event_query = StandaloneEventQueryBridge::new(self.domain_stores.clone());

        let router = tonic::transport::Server::builder()
            .layer(crate::transport::grpc_trace_layer())
            .add_service(CommandGatewayServer::new(gateway))
            .add_service(EventQueryServer::new(event_query));

        match &self.gateway_config {
            GatewayConfig::None => {}
            GatewayConfig::Tcp(port) => {
                let addr: std::net::SocketAddr = format!("0.0.0.0:{port}").parse()?;
                let listener = tokio::net::TcpListener::bind(addr).await?;
                let local_addr = listener.local_addr()?;

                info!(addr = %local_addr, "Gateway listening on TCP");

                let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
                let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

                tokio::spawn(async move {
                    let server = router.serve_with_incoming_shutdown(incoming, async {
                        let _ = shutdown_rx.await;
                    });
                    if let Err(e) = server.await {
                        error!(error = %e, "Gateway server error");
                    }
                });

                self.servers.push(super::server::ServerInfo::from_parts(
                    local_addr,
                    shutdown_tx,
                ));
            }
            GatewayConfig::Uds(path) => {
                let _guard = crate::transport::prepare_uds_socket(path)?;
                let uds = tokio::net::UnixListener::bind(path)?;
                let stream = tokio_stream::wrappers::UnixListenerStream::new(uds);

                info!(path = %path.display(), "Gateway listening on UDS");

                let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

                tokio::spawn(async move {
                    let server = router.serve_with_incoming_shutdown(stream, async {
                        let _ = shutdown_rx.await;
                    });
                    if let Err(e) = server.await {
                        error!(error = %e, "Gateway server error");
                    }
                });

                self.servers.push(super::server::ServerInfo::from_parts(
                    "0.0.0.0:0".parse().unwrap(),
                    shutdown_tx,
                ));
            }
        }

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
