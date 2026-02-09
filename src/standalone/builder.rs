//! Runtime builder for standalone mode.
//!
//! Provides a fluent API for configuring and building the standalone runtime.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::bus::{ChannelConfig, ChannelEventBus, EventBus, MessagingConfig, MessagingType};
#[cfg(feature = "topology")]
use crate::handlers::projectors::topology::TopologyProjector;
use crate::storage::{SqliteConfig, StorageConfig, StorageType};
use crate::transport::{TransportConfig, TransportType, UdsConfig};

use super::runtime::Runtime;
use super::traits::{
    AggregateHandler, ProcessManagerConfig, ProcessManagerHandler, ProjectorConfig,
    ProjectorHandler, SagaConfig, SagaHandler,
};

/// Configuration for optional gateway exposure.
#[derive(Debug, Clone, Default)]
pub enum GatewayConfig {
    /// No gateway (in-process only).
    #[default]
    None,
    /// TCP gateway on specified port.
    Tcp(u16),
    /// UDS gateway at specified path.
    Uds(PathBuf),
}

impl GatewayConfig {
    /// Create a TCP gateway configuration.
    pub fn tcp(port: u16) -> Self {
        Self::Tcp(port)
    }

    /// Create a UDS gateway configuration.
    pub fn uds(path: impl Into<PathBuf>) -> Self {
        Self::Uds(path.into())
    }
}

/// Builder for creating a standalone runtime.
///
/// # Example
///
/// ```ignore
/// use angzarr::standalone::{RuntimeBuilder, ProjectorConfig};
///
/// let runtime = RuntimeBuilder::new()
///     .with_sqlite_memory()
///     .register_aggregate("orders", MyOrdersHandler)
///     .register_projector("accounting", MyProjector, ProjectorConfig::sync())
///     .build()
///     .await?;
/// ```
pub struct RuntimeBuilder {
    /// Default storage configuration (used when no per-domain config).
    storage: StorageConfig,
    /// Per-domain storage configurations.
    domain_storage: HashMap<String, StorageConfig>,
    /// Messaging configuration.
    messaging: MessagingConfig,
    /// Transport configuration.
    transport: TransportConfig,
    /// Gateway configuration.
    gateway: GatewayConfig,
    /// Registered aggregate handlers by domain.
    aggregates: HashMap<String, Arc<dyn AggregateHandler>>,
    /// Registered projector handlers by name.
    projectors: HashMap<String, (Arc<dyn ProjectorHandler>, ProjectorConfig)>,
    /// Registered saga handlers by name.
    sagas: HashMap<String, (Arc<dyn SagaHandler>, SagaConfig)>,
    /// Registered process manager handlers by name.
    process_managers: HashMap<String, (Arc<dyn ProcessManagerHandler>, ProcessManagerConfig)>,
    /// Optional custom event bus (for testing or external transports).
    custom_event_bus: Option<Arc<dyn EventBus>>,
    /// Optional topology projector for automatic descriptor registration.
    #[cfg(feature = "topology")]
    topology_projector: Option<Arc<TopologyProjector>>,
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeBuilder {
    /// Create a new runtime builder with sensible defaults for local development.
    ///
    /// Defaults:
    /// - Storage: SQLite in-memory
    /// - Messaging: Channel (in-memory pub/sub)
    /// - Transport: UDS at /tmp/angzarr/
    /// - Gateway: None (in-process only)
    pub fn new() -> Self {
        Self {
            storage: StorageConfig {
                storage_type: StorageType::Sqlite,
                sqlite: SqliteConfig::default(),
                ..Default::default()
            },
            domain_storage: HashMap::new(),
            messaging: MessagingConfig {
                messaging_type: MessagingType::Channel,
                ..Default::default()
            },
            transport: TransportConfig {
                transport_type: TransportType::Uds,
                uds: UdsConfig::default(),
                ..Default::default()
            },
            gateway: GatewayConfig::None,
            aggregates: HashMap::new(),
            projectors: HashMap::new(),
            sagas: HashMap::new(),
            process_managers: HashMap::new(),
            custom_event_bus: None,
            #[cfg(feature = "topology")]
            topology_projector: None,
        }
    }

    // ========================================================================
    // Storage Configuration
    // ========================================================================

    /// Use SQLite in-memory storage (default).
    pub fn with_sqlite_memory(mut self) -> Self {
        self.storage.storage_type = StorageType::Sqlite;
        self.storage.sqlite.path = None;
        self
    }

    /// Use SQLite file storage.
    pub fn with_sqlite_file(mut self, path: impl Into<String>) -> Self {
        self.storage.storage_type = StorageType::Sqlite;
        self.storage.sqlite.path = Some(path.into());
        self
    }

    /// Use PostgreSQL storage.
    pub fn with_postgres(mut self, uri: impl Into<String>) -> Self {
        self.storage.storage_type = StorageType::Postgres;
        self.storage.postgres.uri = uri.into();
        self
    }

    /// Use MongoDB storage.
    pub fn with_mongodb(mut self, uri: impl Into<String>, database: impl Into<String>) -> Self {
        self.storage.storage_type = StorageType::Mongodb;
        self.storage.mongodb.uri = uri.into();
        self.storage.mongodb.database = database.into();
        self
    }

    /// Use custom storage configuration.
    pub fn with_storage(mut self, config: StorageConfig) -> Self {
        self.storage = config;
        self
    }

    /// Set storage configuration for a specific domain.
    ///
    /// This allows each domain to have its own isolated storage.
    /// Domains without explicit storage configs will use the default.
    pub fn with_domain_storage(mut self, domain: impl Into<String>, config: StorageConfig) -> Self {
        self.domain_storage.insert(domain.into(), config);
        self
    }

    // ========================================================================
    // Messaging Configuration
    // ========================================================================

    /// Use channel messaging (in-memory, default).
    pub fn with_channel_messaging(mut self) -> Self {
        self.messaging.messaging_type = MessagingType::Channel;
        self
    }

    /// Use AMQP messaging.
    pub fn with_amqp(mut self, url: impl Into<String>) -> Self {
        self.messaging.messaging_type = MessagingType::Amqp;
        self.messaging.amqp.url = url.into();
        self
    }

    /// Use Kafka messaging.
    pub fn with_kafka(mut self, bootstrap_servers: impl Into<String>) -> Self {
        self.messaging.messaging_type = MessagingType::Kafka;
        self.messaging.kafka.bootstrap_servers = bootstrap_servers.into();
        self
    }

    /// Use custom messaging configuration.
    pub fn with_messaging(mut self, config: MessagingConfig) -> Self {
        self.messaging = config;
        self
    }

    // ========================================================================
    // Transport Configuration
    // ========================================================================

    /// Use UDS transport (default for local dev).
    pub fn with_uds(mut self) -> Self {
        self.transport.transport_type = TransportType::Uds;
        self
    }

    /// Use UDS transport with custom base path.
    pub fn with_uds_path(mut self, base_path: impl Into<PathBuf>) -> Self {
        self.transport.transport_type = TransportType::Uds;
        self.transport.uds.base_path = base_path.into();
        self
    }

    /// Use TCP transport.
    pub fn with_tcp(mut self) -> Self {
        self.transport.transport_type = TransportType::Tcp;
        self
    }

    /// Use custom transport configuration.
    pub fn with_transport(mut self, config: TransportConfig) -> Self {
        self.transport = config;
        self
    }

    // ========================================================================
    // Gateway Configuration
    // ========================================================================

    /// Expose gateway for external clients via TCP.
    pub fn with_gateway_tcp(mut self, port: u16) -> Self {
        self.gateway = GatewayConfig::Tcp(port);
        self
    }

    /// Expose gateway for external clients via UDS.
    pub fn with_gateway_uds(mut self, path: impl Into<PathBuf>) -> Self {
        self.gateway = GatewayConfig::Uds(path.into());
        self
    }

    /// Use custom gateway configuration.
    pub fn with_gateway(mut self, config: GatewayConfig) -> Self {
        self.gateway = config;
        self
    }

    // ========================================================================
    // Custom Event Bus (for testing)
    // ========================================================================

    /// Use a custom event bus implementation.
    ///
    /// The bus is used for both publishing events and creating subscribers.
    /// Each bus transport (Channel, IPC, AMQP, Kafka) implements
    /// `create_subscriber()` to produce transport-appropriate subscribers.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use angzarr::bus::{ChannelEventBus, ChannelConfig, LossyEventBus, LossyConfig};
    ///
    /// let channel_bus = Arc::new(ChannelEventBus::new(ChannelConfig::publisher()));
    /// let lossy_bus = Arc::new(LossyEventBus::new(
    ///     channel_bus.clone(),
    ///     LossyConfig::with_drop_rate(0.5),
    /// ));
    ///
    /// let runtime = RuntimeBuilder::new()
    ///     .with_event_bus(lossy_bus)
    ///     .register_aggregate("orders", handler)
    ///     .build()
    ///     .await?;
    /// ```
    pub fn with_event_bus(mut self, event_bus: Arc<dyn EventBus>) -> Self {
        self.custom_event_bus = Some(event_bus);
        self
    }

    // ========================================================================
    // Handler Registration
    // ========================================================================

    /// Register an aggregate handler for a domain.
    ///
    /// Each domain can have one aggregate handler that processes commands
    /// and returns events.
    pub fn register_aggregate<H>(mut self, domain: impl Into<String>, handler: H) -> Self
    where
        H: AggregateHandler,
    {
        self.aggregates.insert(domain.into(), Arc::new(handler));
        self
    }

    /// Register a projector handler.
    ///
    /// Projectors receive events and update read models.
    /// Configuration specifies whether the projector is synchronous
    /// (blocks command response) or asynchronous (background).
    pub fn register_projector<H>(
        mut self,
        name: impl Into<String>,
        handler: H,
        config: ProjectorConfig,
    ) -> Self
    where
        H: ProjectorHandler,
    {
        self.projectors
            .insert(name.into(), (Arc::new(handler), config));
        self
    }

    /// Register a saga handler.
    ///
    /// Sagas receive events and can emit commands to other aggregates
    /// for orchestrating cross-aggregate workflows.
    pub fn register_saga<H>(
        mut self,
        name: impl Into<String>,
        handler: H,
        config: SagaConfig,
    ) -> Self
    where
        H: SagaHandler,
    {
        self.sagas.insert(name.into(), (Arc::new(handler), config));
        self
    }

    /// Register a process manager handler.
    ///
    /// Process managers are aggregates that coordinate across domains.
    /// They subscribe to events, maintain event-sourced state in their
    /// own domain, and produce commands for other aggregates.
    ///
    /// The PM domain gets its own aggregate storage automatically.
    pub fn register_process_manager<H>(
        mut self,
        name: impl Into<String>,
        handler: H,
        config: ProcessManagerConfig,
    ) -> Self
    where
        H: ProcessManagerHandler,
    {
        self.process_managers
            .insert(name.into(), (Arc::new(handler), config));
        self
    }

    // ========================================================================
    // Topology
    // ========================================================================

    /// Register the topology projector.
    ///
    /// Registers it as an async projector for event observation AND calls
    /// `register_components()` during build with all collected descriptors.
    /// This ensures sagas, process managers, and projectors appear as correctly
    /// typed nodes in the topology graph.
    #[cfg(feature = "topology")]
    pub fn register_topology(
        mut self,
        projector: Arc<TopologyProjector>,
        config: ProjectorConfig,
    ) -> Self {
        self.topology_projector = Some(projector.clone());
        self.projectors
            .insert("topology".to_string(), (projector, config));
        self
    }

    // ========================================================================
    // Build
    // ========================================================================

    /// Build the runtime.
    ///
    /// Initializes storage, messaging, and transport based on configuration.
    /// Returns a Runtime that can be used to run the system.
    pub async fn build(mut self) -> Result<Runtime, Box<dyn std::error::Error>> {
        // Auto-register the _angzarr meta aggregate for component registration
        use super::meta_aggregate::{MetaAggregateHandler, META_DOMAIN};
        self.aggregates.insert(
            META_DOMAIN.to_string(),
            Arc::new(MetaAggregateHandler::new()),
        );

        // Use custom event bus if provided, otherwise create a default channel bus.
        let event_bus: Arc<dyn EventBus> = self
            .custom_event_bus
            .unwrap_or_else(|| Arc::new(ChannelEventBus::new(ChannelConfig::publisher())));

        Runtime::new(
            self.storage,
            self.domain_storage,
            self.messaging,
            self.transport,
            self.gateway,
            self.aggregates,
            self.projectors,
            self.sagas,
            self.process_managers,
            event_bus,
            #[cfg(feature = "topology")]
            self.topology_projector,
        )
        .await
    }

    // ========================================================================
    // Accessors (for testing/inspection)
    // ========================================================================

    /// Get the storage configuration.
    pub fn storage_config(&self) -> &StorageConfig {
        &self.storage
    }

    /// Get the messaging configuration.
    pub fn messaging_config(&self) -> &MessagingConfig {
        &self.messaging
    }

    /// Get the transport configuration.
    pub fn transport_config(&self) -> &TransportConfig {
        &self.transport
    }

    /// Get the registered aggregate domains.
    pub fn aggregate_domains(&self) -> Vec<&str> {
        self.aggregates.keys().map(|s| s.as_str()).collect()
    }

    /// Get the registered projector names.
    pub fn projector_names(&self) -> Vec<&str> {
        self.projectors.keys().map(|s| s.as_str()).collect()
    }

    /// Get the registered saga names.
    pub fn saga_names(&self) -> Vec<&str> {
        self.sagas.keys().map(|s| s.as_str()).collect()
    }

    /// Get the registered process manager names.
    pub fn process_manager_names(&self) -> Vec<&str> {
        self.process_managers.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let builder = RuntimeBuilder::new();

        assert_eq!(builder.storage.storage_type, StorageType::Sqlite);
        assert!(builder.storage.sqlite.path.is_none());
        assert_eq!(builder.messaging.messaging_type, MessagingType::Channel);
        assert_eq!(builder.transport.transport_type, TransportType::Uds);
    }

    #[test]
    fn test_builder_sqlite_file() {
        let builder = RuntimeBuilder::new().with_sqlite_file("./data/events.db");

        assert_eq!(builder.storage.storage_type, StorageType::Sqlite);
        assert_eq!(
            builder.storage.sqlite.path,
            Some("./data/events.db".to_string())
        );
    }

    #[test]
    fn test_builder_postgres() {
        let builder = RuntimeBuilder::new().with_postgres("postgres://localhost:5432/test");

        assert_eq!(builder.storage.storage_type, StorageType::Postgres);
        assert_eq!(
            builder.storage.postgres.uri,
            "postgres://localhost:5432/test"
        );
    }

    #[test]
    fn test_builder_amqp() {
        let builder = RuntimeBuilder::new().with_amqp("amqp://localhost:5672");

        assert_eq!(builder.messaging.messaging_type, MessagingType::Amqp);
        assert_eq!(builder.messaging.amqp.url, "amqp://localhost:5672");
    }

    #[test]
    fn test_builder_gateway_tcp() {
        let builder = RuntimeBuilder::new().with_gateway_tcp(50051);

        match builder.gateway {
            GatewayConfig::Tcp(port) => assert_eq!(port, 50051),
            _ => panic!("Expected TCP gateway"),
        }
    }

    #[test]
    fn test_builder_gateway_uds() {
        let builder = RuntimeBuilder::new().with_gateway_uds("/tmp/gateway.sock");

        match builder.gateway {
            GatewayConfig::Uds(path) => assert_eq!(path, PathBuf::from("/tmp/gateway.sock")),
            _ => panic!("Expected UDS gateway"),
        }
    }
}
