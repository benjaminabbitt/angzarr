//! Runtime implementation for embedded mode.
//!
//! Orchestrates storage, messaging, and handlers for local development.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::bus::{ChannelEventBus, EventBus, MessagingConfig, PublishResult};
use crate::proto::EventBook;
use crate::storage::{EventStore, SnapshotStore, StorageConfig};
use crate::transport::TransportConfig;

use super::builder::GatewayConfig;
use super::client::CommandClient;
use super::router::CommandRouter;
use super::traits::{
    AggregateHandler, ProjectorConfig, ProjectorHandler, SagaConfig, SagaHandler,
};

/// Embedded runtime for angzarr.
///
/// Manages all components for running angzarr locally:
/// - Storage (events and snapshots)
/// - Event bus (for pub/sub)
/// - Aggregate handlers (business logic)
/// - Projector handlers (read models)
/// - Saga handlers (cross-aggregate workflows)
/// - Optional gateway (for external clients)
pub struct Runtime {
    /// Event store.
    event_store: Arc<dyn EventStore>,
    /// Snapshot store.
    snapshot_store: Arc<dyn SnapshotStore>,
    /// Channel event bus for subscription (internal pub/sub).
    channel_bus: Arc<ChannelEventBus>,
    /// Event bus for publishing (may be wrapped with lossy).
    event_bus: Arc<dyn EventBus>,
    /// Command router for dispatching commands to aggregates.
    router: Arc<CommandRouter>,
    /// Projector handlers.
    projectors: Arc<RwLock<Vec<ProjectorEntry>>>,
    /// Saga handlers.
    sagas: Arc<RwLock<Vec<SagaEntry>>>,
    /// Background task handles.
    tasks: Vec<JoinHandle<()>>,
    /// Gateway configuration.
    gateway_config: GatewayConfig,
    /// Transport configuration.
    transport_config: TransportConfig,
}

/// Entry for a registered projector.
struct ProjectorEntry {
    name: String,
    handler: Arc<dyn ProjectorHandler>,
    config: ProjectorConfig,
}

/// Entry for a registered saga.
struct SagaEntry {
    name: String,
    handler: Arc<dyn SagaHandler>,
    #[allow(dead_code)]
    config: SagaConfig,
}

impl Runtime {
    /// Create a new runtime (called by RuntimeBuilder).
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new(
        storage_config: StorageConfig,
        _messaging_config: MessagingConfig,
        transport_config: TransportConfig,
        gateway_config: GatewayConfig,
        aggregates: HashMap<String, Arc<dyn AggregateHandler>>,
        projectors: HashMap<String, (Arc<dyn ProjectorHandler>, ProjectorConfig)>,
        sagas: HashMap<String, (Arc<dyn SagaHandler>, SagaConfig)>,
        channel_bus: Arc<ChannelEventBus>,
        event_bus: Arc<dyn EventBus>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize storage
        let (event_store, snapshot_store) = crate::storage::init_storage(&storage_config).await?;

        info!(
            storage_type = ?storage_config.storage_type,
            aggregates = aggregates.len(),
            projectors = projectors.len(),
            sagas = sagas.len(),
            "Runtime initialized"
        );

        // Create command router with the (possibly lossy) event bus
        let router = Arc::new(CommandRouter::new(
            aggregates,
            event_store.clone(),
            snapshot_store.clone(),
            event_bus.clone(),
        ));

        // Convert projectors to entries
        let projector_entries: Vec<ProjectorEntry> = projectors
            .into_iter()
            .map(|(name, (handler, config))| ProjectorEntry {
                name,
                handler,
                config,
            })
            .collect();

        // Convert sagas to entries
        let saga_entries: Vec<SagaEntry> = sagas
            .into_iter()
            .map(|(name, (handler, config))| SagaEntry {
                name,
                handler,
                config,
            })
            .collect();

        Ok(Self {
            event_store,
            snapshot_store,
            channel_bus,
            event_bus,
            router,
            projectors: Arc::new(RwLock::new(projector_entries)),
            sagas: Arc::new(RwLock::new(saga_entries)),
            tasks: Vec::new(),
            gateway_config,
            transport_config,
        })
    }

    /// Get a command client for programmatic command submission.
    ///
    /// The client can be cloned and shared across tasks.
    pub fn command_client(&self) -> CommandClient {
        CommandClient::new(self.router.clone())
    }

    /// Get access to the event store.
    pub fn event_store(&self) -> Arc<dyn EventStore> {
        self.event_store.clone()
    }

    /// Get access to the snapshot store.
    pub fn snapshot_store(&self) -> Arc<dyn SnapshotStore> {
        self.snapshot_store.clone()
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

    /// Run the runtime until Ctrl+C.
    ///
    /// This starts all background tasks and waits for shutdown signal.
    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Start event distribution
        self.start_event_distribution().await?;

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

        Ok(())
    }

    /// Start event distribution to projectors and sagas.
    async fn start_event_distribution(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let projectors = self.projectors.clone();
        let sagas = self.sagas.clone();
        let router = self.router.clone();

        // Subscribe to events from the channel bus (after lossy layer)
        let subscriber = self.channel_bus.with_config(crate::bus::ChannelConfig::subscriber_all());

        // Create handler that distributes events
        let handler = EventDistributionHandler {
            projectors,
            sagas,
            router,
        };

        subscriber.subscribe(Box::new(handler)).await?;
        subscriber.start_consuming().await?;

        info!("Event distribution started");

        Ok(())
    }

    /// Start the gateway server.
    async fn start_gateway(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Gateway implementation will be added in Phase 4
        // For now, just log that it would start
        match &self.gateway_config {
            GatewayConfig::None => {}
            GatewayConfig::Tcp(port) => {
                info!(port = %port, "Gateway would start on TCP (not yet implemented)");
            }
            GatewayConfig::Uds(path) => {
                info!(
                    path = %path.display(),
                    transport = ?self.transport_config.transport_type,
                    "Gateway would start on UDS (not yet implemented)"
                );
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

/// Handler for distributing events to projectors and sagas.
struct EventDistributionHandler {
    projectors: Arc<RwLock<Vec<ProjectorEntry>>>,
    sagas: Arc<RwLock<Vec<SagaEntry>>>,
    router: Arc<CommandRouter>,
}

impl crate::bus::EventHandler for EventDistributionHandler {
    fn handle(
        &self,
        book: Arc<EventBook>,
    ) -> futures::future::BoxFuture<'static, Result<(), crate::bus::BusError>> {
        let projectors = self.projectors.clone();
        let sagas = self.sagas.clone();
        let router = self.router.clone();

        Box::pin(async move {
            let domain = book
                .cover
                .as_ref()
                .map(|c| c.domain.as_str())
                .unwrap_or("unknown");

            // Process async projectors
            let projector_list = projectors.read().await;
            for entry in projector_list.iter() {
                // Skip synchronous projectors (already processed)
                if entry.config.synchronous {
                    continue;
                }

                // Check domain filter
                if !entry.config.domains.is_empty()
                    && !entry.config.domains.iter().any(|d| d == domain)
                {
                    continue;
                }

                if let Err(e) = entry.handler.handle(&book).await {
                    error!(
                        projector = %entry.name,
                        domain = %domain,
                        error = %e,
                        "Async projector failed"
                    );
                }
            }

            // Process sagas
            let saga_list = sagas.read().await;
            for entry in saga_list.iter() {
                // Check domain filter
                if !entry.config.domains.is_empty()
                    && !entry.config.domains.iter().any(|d| d == domain)
                {
                    continue;
                }

                match entry.handler.handle(&book).await {
                    Ok(response) => {
                        // Execute saga commands
                        for command in response.commands {
                            if let Err(e) = router.execute_command(command).await {
                                error!(
                                    saga = %entry.name,
                                    error = %e,
                                    "Saga command execution failed"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            saga = %entry.name,
                            domain = %domain,
                            error = %e,
                            "Saga handler failed"
                        );
                    }
                }
            }

            Ok(())
        })
    }
}
