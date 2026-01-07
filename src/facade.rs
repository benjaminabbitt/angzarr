//! Evented facade for in-process library usage.
//!
//! Provides a simple API for using evented-rs as an embedded library
//! without requiring gRPC servers or external services.
//!
//! # Example
//!
//! ```ignore
//! use evented::facade::{Evented, EventedConfig};
//! use evented::interfaces::{BusinessLogicClient, Projector};
//!
//! // Create evented instance
//! let evented = Evented::builder(EventedConfig::in_memory())
//!     .with_business_logic(MyBusinessLogic::new())
//!     .with_projector(MyProjector::new())
//!     .build()
//!     .await?;
//!
//! // Send a command
//! let response = evented.handle_command(command_book).await?;
//!
//! // Query events
//! let events = evented.get_events("orders", aggregate_id).await?;
//! ```

use std::sync::Arc;

use uuid::Uuid;

use crate::bus::InProcessEventBus;
use crate::interfaces::{
    BusinessLogicClient, EventBus, EventStore, Projector, Saga, StorageError,
};
use crate::proto::{CommandBook, EventBook, SynchronousProcessingResponse};
use crate::repository::EventBookRepository;
use crate::storage::{SqliteEventStore, SqliteSnapshotStore};

/// Configuration for Evented instance.
#[derive(Debug, Clone)]
pub struct EventedConfig {
    /// SQLite database path. Use `:memory:` for in-memory.
    pub database_path: String,
}

impl EventedConfig {
    /// Create config for in-memory database (testing/embedded).
    pub fn in_memory() -> Self {
        Self {
            database_path: ":memory:".to_string(),
        }
    }

    /// Create config with file-based database.
    pub fn with_database(path: impl Into<String>) -> Self {
        Self {
            database_path: path.into(),
        }
    }
}

impl Default for EventedConfig {
    fn default() -> Self {
        Self::in_memory()
    }
}

/// Builder for Evented instance.
pub struct EventedBuilder {
    config: EventedConfig,
    business_logic: Option<Arc<dyn BusinessLogicClient>>,
    projectors: Vec<Box<dyn Projector>>,
    sagas: Vec<Box<dyn Saga>>,
}

impl EventedBuilder {
    /// Create a new builder with given config.
    pub fn new(config: EventedConfig) -> Self {
        Self {
            config,
            business_logic: None,
            projectors: Vec::new(),
            sagas: Vec::new(),
        }
    }

    /// Set the business logic handler.
    pub fn with_business_logic(mut self, logic: impl BusinessLogicClient + 'static) -> Self {
        self.business_logic = Some(Arc::new(logic));
        self
    }

    /// Add an in-process projector.
    pub fn with_projector(mut self, projector: impl Projector + 'static) -> Self {
        self.projectors.push(Box::new(projector));
        self
    }

    /// Add an in-process saga.
    pub fn with_saga(mut self, saga: impl Saga + 'static) -> Self {
        self.sagas.push(Box::new(saga));
        self
    }

    /// Build the Evented instance.
    pub async fn build(self) -> Result<Evented, EventedError> {
        // Connect to database
        let db_url = if self.config.database_path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            format!("sqlite:{}?mode=rwc", self.config.database_path)
        };

        let pool = sqlx::SqlitePool::connect(&db_url).await?;

        // Initialize stores
        let event_store = Arc::new(SqliteEventStore::new(pool.clone()));
        event_store.init().await?;

        let snapshot_store = Arc::new(SqliteSnapshotStore::new(pool));
        snapshot_store.init().await?;

        // Create repository
        let repository = Arc::new(EventBookRepository::new(
            event_store.clone(),
            snapshot_store.clone(),
        ));

        // Create event bus and register handlers
        let event_bus = Arc::new(InProcessEventBus::new());
        for projector in self.projectors {
            event_bus.add_projector(projector).await;
        }
        for saga in self.sagas {
            event_bus.add_saga(saga).await;
        }

        // Use placeholder if no business logic provided
        let business_logic = self.business_logic.unwrap_or_else(|| {
            Arc::new(crate::clients::PlaceholderBusinessLogic::with_defaults())
        });

        Ok(Evented {
            event_store,
            snapshot_store,
            repository,
            event_bus,
            business_logic,
        })
    }
}

/// Main evented instance for library usage.
///
/// Provides a high-level API for event sourcing without gRPC.
pub struct Evented {
    event_store: Arc<SqliteEventStore>,
    snapshot_store: Arc<SqliteSnapshotStore>,
    repository: Arc<EventBookRepository>,
    event_bus: Arc<InProcessEventBus>,
    business_logic: Arc<dyn BusinessLogicClient>,
}

impl Evented {
    /// Create a new builder with given config.
    pub fn builder(config: EventedConfig) -> EventedBuilder {
        EventedBuilder::new(config)
    }

    /// Handle a command and return the response.
    ///
    /// This is the main entry point for command processing:
    /// 1. Loads prior events for the aggregate
    /// 2. Calls business logic with contextual command
    /// 3. Persists resulting events
    /// 4. Notifies projectors and sagas
    pub async fn handle_command(
        &self,
        command: CommandBook,
    ) -> Result<SynchronousProcessingResponse, EventedError> {
        /// Maximum depth for saga command processing to prevent runaway chains.
        const MAX_SAGA_DEPTH: usize = 100;

        let mut all_books = Vec::new();
        let mut command_queue = vec![command];

        while let Some(cmd) = command_queue.pop() {
            if all_books.len() >= MAX_SAGA_DEPTH {
                return Err(EventedError::SagaDepthExceeded {
                    depth: all_books.len(),
                    max: MAX_SAGA_DEPTH,
                });
            }

            // Extract aggregate identity
            let cover = cmd.cover.as_ref().ok_or(EventedError::MissingCover)?;

            let domain = cover.domain.clone();
            let root = cover.root.as_ref().ok_or(EventedError::MissingRoot)?;

            let root_uuid =
                Uuid::from_slice(&root.value).map_err(EventedError::InvalidUuid)?;

            // Load prior state
            let prior_events = self.repository.get(&domain, root_uuid).await?;

            // Create contextual command
            let contextual = crate::proto::ContextualCommand {
                events: Some(prior_events),
                command: Some(cmd),
            };

            // Call business logic
            let new_events = self.business_logic.handle(&domain, contextual).await?;

            // Persist events
            self.repository.put(&new_events).await?;

            // Wrap in Arc for immutable distribution
            let new_events = Arc::new(new_events);

            // Notify event bus (zero-copy sharing via Arc)
            self.event_bus.publish(Arc::clone(&new_events)).await?;

            all_books.push(Arc::try_unwrap(new_events).unwrap_or_else(|arc| (*arc).clone()));

            // Queue any saga-produced commands for processing
            let pending_commands = self.event_bus.take_pending_commands().await;
            command_queue.extend(pending_commands);
        }

        Ok(SynchronousProcessingResponse {
            books: all_books,
            projections: vec![],
        })
    }

    /// Record events directly (bypass business logic).
    ///
    /// Used by sagas or for event replay.
    pub async fn record_events(
        &self,
        events: EventBook,
    ) -> Result<SynchronousProcessingResponse, EventedError> {
        self.repository.put(&events).await?;

        let events = Arc::new(events);
        self.event_bus.publish(Arc::clone(&events)).await?;

        Ok(SynchronousProcessingResponse {
            books: vec![Arc::try_unwrap(events).unwrap_or_else(|arc| (*arc).clone())],
            projections: vec![],
        })
    }

    /// Get all events for an aggregate.
    pub async fn get_events(&self, domain: &str, root: Uuid) -> Result<EventBook, EventedError> {
        Ok(self.repository.get(domain, root).await?)
    }

    /// Get events in a range.
    pub async fn get_events_range(
        &self,
        domain: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<EventBook, EventedError> {
        Ok(self.repository.get_from_to(domain, root, from, to).await?)
    }

    /// List all aggregate roots in a domain.
    pub async fn list_aggregates(&self, domain: &str) -> Result<Vec<Uuid>, EventedError> {
        Ok(self.event_store.list_roots(domain).await?)
    }

    /// Get direct access to the event store.
    pub fn event_store(&self) -> &Arc<SqliteEventStore> {
        &self.event_store
    }

    /// Get direct access to the snapshot store.
    pub fn snapshot_store(&self) -> &Arc<SqliteSnapshotStore> {
        &self.snapshot_store
    }

    /// Get direct access to the event bus.
    pub fn event_bus(&self) -> &Arc<InProcessEventBus> {
        &self.event_bus
    }
}

/// Errors from Evented operations.
#[derive(Debug, thiserror::Error)]
pub enum EventedError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Business logic error: {0}")]
    BusinessLogic(#[from] crate::interfaces::BusinessError),

    #[error("Event bus error: {0}")]
    EventBus(#[from] crate::interfaces::BusError),

    #[error("Cover missing from CommandBook")]
    MissingCover,

    #[error("Root UUID missing from Cover")]
    MissingRoot,

    #[error("Invalid UUID: {0}")]
    InvalidUuid(#[source] uuid::Error),

    #[error("Saga command depth exceeded: {depth} >= {max}")]
    SagaDepthExceeded { depth: usize, max: usize },
}
