//! Runtime implementation for embedded mode.
//!
//! Orchestrates storage, messaging, and handlers for local development.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::bus::{ChannelConfig, ChannelEventBus, EventBus, MessagingConfig, PublishResult};
use crate::discovery::k8s::K8sServiceDiscovery;
use crate::discovery::ServiceDiscovery;
use crate::handlers::core::{ProcessManagerEventHandler, ProjectorEventHandler, SagaEventHandler};
use crate::orchestration::command::local::LocalCommandExecutor;
use crate::orchestration::destination::local::LocalDestinationFetcher;
use crate::orchestration::process_manager::local::LocalPMContextFactory;
use crate::orchestration::projector::local::LocalProjectorContext;
use crate::orchestration::saga::local::LocalSagaContextFactory;
use crate::proto::aggregate_client::AggregateClient;
use crate::proto::{CommandBook, EventBook};
use crate::proto_ext::CoverExt;
use crate::storage::{EventStore, SnapshotStore, StorageConfig};
use crate::transport::TransportConfig;

use super::builder::GatewayConfig;
use super::client::CommandClient;
use super::router::{CommandRouter, DomainStorage};
use super::server::{start_aggregate_server, start_projector_server, ServerInfo};
use super::traits::{
    AggregateHandler, ProcessManagerConfig, ProcessManagerHandler, ProjectorConfig,
    ProjectorHandler, SagaConfig, SagaHandler,
};

/// Embedded runtime for angzarr.
///
/// Manages all components for running angzarr locally:
/// - Storage (events and snapshots per domain)
/// - Event bus (for pub/sub)
/// - Aggregate handlers (business logic)
/// - Projector handlers (read models)
/// - Saga handlers (cross-aggregate workflows)
/// - Optional gateway (for external clients)
pub struct Runtime {
    /// Per-domain storage.
    domain_stores: HashMap<String, DomainStorage>,
    /// Channel event bus for subscription (internal pub/sub).
    channel_bus: Arc<ChannelEventBus>,
    /// Event bus for publishing (may be wrapped with lossy).
    event_bus: Arc<dyn EventBus>,
    /// Command router for dispatching commands to aggregates.
    router: Arc<CommandRouter>,
    /// Service discovery for projectors.
    #[allow(dead_code)]
    discovery: Arc<dyn ServiceDiscovery>,
    /// Projector entries (needed for sync projector iteration in publish_events).
    projectors: Arc<RwLock<Vec<ProjectorEntry>>>,
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
        channel_bus: Arc<ChannelEventBus>,
        event_bus: Arc<dyn EventBus>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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

        // Start gRPC servers for each aggregate and create clients
        let mut servers = Vec::new();
        let mut business_clients = HashMap::new();

        for (domain, handler) in aggregates {
            let server_info = start_aggregate_server(&domain, handler)
                .await
                .map_err(|e| format!("Failed to start aggregate server for {}: {}", domain, e))?;

            let addr = format!("http://{}", server_info.addr);
            let client = AggregateClient::connect(addr)
                .await
                .map_err(|e| format!("Failed to connect to aggregate {}: {}", domain, e))?;

            business_clients.insert(domain, Arc::new(Mutex::new(client)));
            servers.push(server_info);
        }

        // Create service discovery for projectors
        let discovery: Arc<dyn ServiceDiscovery> = Arc::new(K8sServiceDiscovery::new_static());

        // Start gRPC servers for sync projectors and register with discovery
        for (name, (handler, config)) in &projectors {
            if config.synchronous {
                let server_info = start_projector_server(name, handler.clone())
                    .await
                    .map_err(|e| format!("Failed to start projector server for {}: {}", name, e))?;

                // Register with service discovery
                // Use first domain or "default" if no domain filter
                let domain = config
                    .domains
                    .first()
                    .map(|s| s.as_str())
                    .unwrap_or("default");
                discovery
                    .register_projector(
                        name,
                        domain,
                        &server_info.addr.ip().to_string(),
                        server_info.addr.port(),
                    )
                    .await;

                servers.push(server_info);
            }
        }

        // Create command router with gRPC clients
        let router = Arc::new(CommandRouter::new(
            business_clients,
            domain_stores.clone(),
            discovery.clone(),
            event_bus.clone(),
        ));

        // Convert projectors to entries (kept for sync projector iteration in publish_events)
        let projector_entries: Vec<ProjectorEntry> = projectors
            .into_iter()
            .map(|(name, (handler, config))| ProjectorEntry {
                name,
                handler,
                config,
            })
            .collect();

        // Start event distribution for sagas, PMs, and async projectors
        let executor = Arc::new(LocalCommandExecutor::new(router.clone()));
        let fetcher = Arc::new(LocalDestinationFetcher::new(domain_stores.clone()));

        // Async projectors — each gets its own subscriber
        for entry in &projector_entries {
            let ctx = Arc::new(LocalProjectorContext::new(entry.handler.clone()));
            let handler = ProjectorEventHandler::with_config(
                ctx,
                None,
                entry.config.domains.clone(),
                entry.config.synchronous,
            );
            let sub = channel_bus.with_config(ChannelConfig::subscriber_all());
            sub.subscribe(Box::new(handler)).await?;
            sub.start_consuming().await?;
        }

        // Sagas — domain-filtered subscribers
        for (name, (handler, config)) in sagas {
            let factory = Arc::new(LocalSagaContextFactory::new(handler));
            let validator = build_output_domain_validator(&name, &config.output_domains);
            let handler = SagaEventHandler::from_factory_with_validator(
                factory,
                executor.clone(),
                Some(fetcher.clone()),
                Some(Arc::new(validator)),
                crate::utils::retry::RetryConfig::for_saga_commands(),
            );
            let sub = channel_bus.with_config(ChannelConfig::subscriber(config.input_domain));
            sub.subscribe(Box::new(handler)).await?;
            sub.start_consuming().await?;
        }

        // Process managers — subscriber_all with handler-level subscription filtering
        for (_name, (handler, config)) in process_managers {
            let subscriptions = handler.subscriptions();
            let pm_store = match domain_stores.get(&config.domain) {
                Some(store) => store.clone(),
                None => continue,
            };
            let factory = Arc::new(LocalPMContextFactory::new(
                handler,
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
            let sub = channel_bus.with_config(ChannelConfig::subscriber_all());
            sub.subscribe(Box::new(pm_handler)).await?;
            sub.start_consuming().await?;
        }

        info!("Event distribution started");

        Ok(Self {
            domain_stores,
            channel_bus,
            event_bus,
            router,
            discovery,
            projectors: Arc::new(RwLock::new(projector_entries)),
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

    /// Get access to the channel bus (for subscription).
    pub fn channel_bus(&self) -> Arc<ChannelEventBus> {
        self.channel_bus.clone()
    }

    /// Get the command router.
    pub fn router(&self) -> Arc<CommandRouter> {
        self.router.clone()
    }

    /// Start the runtime without blocking.
    ///
    /// This starts event distribution to projectors and sagas.
    /// Use this for testing or when you need to interact with the runtime
    /// programmatically after starting.
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Event distribution is now started during construction.
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
                info!("No gateway configured, running in embedded-only mode");
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
    /// 1. Starts an internal bridge server (AggregateCoordinator + EventQuery)
    /// 2. Registers it with discovery as wildcard domain
    /// 3. Creates GatewayService + EventQueryProxy using discovery
    /// 4. Serves both on the configured TCP port or UDS path
    async fn start_gateway(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use std::time::Duration;

        use super::server::start_bridge_server;
        use crate::handlers::gateway::{EventQueryProxy, GatewayService};
        use crate::proto::command_gateway_server::CommandGatewayServer;
        use crate::proto::event_query_server::EventQueryServer;

        // Start internal bridge server on random port
        let bridge_server = start_bridge_server(self.router.clone(), self.domain_stores.clone())
            .await
            .map_err(|e| format!("Failed to start bridge server: {e}"))?;

        let bridge_addr = bridge_server.addr;

        // Register bridge with discovery as wildcard domain
        self.discovery
            .register_aggregate("*", &bridge_addr.ip().to_string(), bridge_addr.port())
            .await;

        self.servers.push(bridge_server);

        // Create gateway service (no streaming in standalone mode)
        let gateway = GatewayService::new(self.discovery.clone(), None, Duration::from_secs(30));
        let event_query_proxy = EventQueryProxy::new(self.discovery.clone());

        // Build the tonic router
        let router = tonic::transport::Server::builder()
            .add_service(CommandGatewayServer::new(gateway))
            .add_service(EventQueryServer::new(event_query_proxy));

        // Serve on configured transport
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

                // UDS doesn't have a meaningful SocketAddr; use the unspecified address
                self.servers.push(super::server::ServerInfo::from_parts(
                    "0.0.0.0:0".parse().unwrap(),
                    shutdown_tx,
                ));
            }
        }

        Ok(())
    }

    /// Publish events and handle synchronous projectors.
    ///
    /// This is used internally by the router after persisting events.
    #[allow(dead_code)]
    pub(crate) async fn publish_events(
        &self,
        events: Arc<EventBook>,
    ) -> Result<PublishResult, Box<dyn std::error::Error>> {
        // Get synchronous projector results
        let projectors = self.projectors.read().await;
        let mut result = PublishResult::default();

        for entry in projectors.iter() {
            if entry.config.synchronous {
                match entry.handler.handle(&events).await {
                    Ok(projection) => {
                        result.projections.push(projection);
                    }
                    Err(e) => {
                        error!(
                            projector = %entry.name,
                            error = %e,
                            "Synchronous projector failed"
                        );
                        return Err(format!("Projector {} failed: {}", entry.name, e).into());
                    }
                }
            }
        }

        // Publish to bus for async consumers
        self.event_bus.publish(events).await?;

        Ok(result)
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
