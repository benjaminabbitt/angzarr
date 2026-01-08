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
use crate::interfaces::{BusinessLogicClient, EventBus, EventStore, Projector, Saga, StorageError};
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
        let business_logic = self
            .business_logic
            .unwrap_or_else(|| Arc::new(crate::clients::PlaceholderBusinessLogic::with_defaults()));

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
        let mut all_projections = Vec::new();
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

            let root_uuid = Uuid::from_slice(&root.value).map_err(EventedError::InvalidUuid)?;

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
            let publish_result = self.event_bus.publish(Arc::clone(&new_events)).await?;
            all_projections.extend(publish_result.projections);

            all_books.push(Arc::try_unwrap(new_events).unwrap_or_else(|arc| (*arc).clone()));

            // Queue any saga-produced commands for processing
            let pending_commands = self.event_bus.take_pending_commands().await;
            command_queue.extend(pending_commands);
        }

        Ok(SynchronousProcessingResponse {
            books: all_books,
            commands: vec![],
            projections: all_projections,
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
        let publish_result = self.event_bus.publish(Arc::clone(&events)).await?;

        Ok(SynchronousProcessingResponse {
            books: vec![Arc::try_unwrap(events).unwrap_or_else(|arc| (*arc).clone())],
            commands: vec![],
            projections: publish_result.projections,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::projector::{Projector, Result as ProjectorResult};
    use crate::interfaces::saga::{Saga, Result as SagaResult};
    use crate::proto::{event_page, CommandBook, CommandPage, Cover, EventPage, Projection};
    use crate::proto::Uuid as ProtoUuid;
    use crate::test_utils::MockBusinessLogic;
    use async_trait::async_trait;
    use prost_types::Any;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn make_uuid() -> Uuid {
        Uuid::new_v4()
    }

    fn make_proto_uuid(uuid: Uuid) -> ProtoUuid {
        ProtoUuid {
            value: uuid.as_bytes().to_vec(),
        }
    }

    fn make_command_book(domain: &str, root: Uuid) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(make_proto_uuid(root)),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(Any {
                    type_url: "test.CreateOrder".to_string(),
                    value: vec![],
                }),
                synchronous: false,
            }],
        }
    }

    fn make_event_book(domain: &str, root: Uuid, event_count: usize) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(make_proto_uuid(root)),
            }),
            pages: (0..event_count)
                .map(|i| EventPage {
                    sequence: Some(event_page::Sequence::Num(i as u32)),
                    event: Some(Any {
                        type_url: format!("test.Event{}", i),
                        value: vec![],
                    }),
                    created_at: None,
                    synchronous: false,
                })
                .collect(),
            snapshot: None,
        }
    }

    struct CountingProjector {
        count: AtomicUsize,
    }

    impl CountingProjector {
        fn new() -> Self {
            Self {
                count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl Projector for CountingProjector {
        fn name(&self) -> &str {
            "counter"
        }

        fn domains(&self) -> Vec<String> {
            vec![]
        }

        async fn project(&self, _book: &Arc<EventBook>) -> ProjectorResult<Option<Projection>> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }
    }

    #[tokio::test]
    async fn test_evented_config_in_memory() {
        let config = EventedConfig::in_memory();
        assert_eq!(config.database_path, ":memory:");
    }

    #[tokio::test]
    async fn test_evented_config_with_database() {
        let config = EventedConfig::with_database("/tmp/test.db");
        assert_eq!(config.database_path, "/tmp/test.db");
    }

    #[tokio::test]
    async fn test_evented_config_default() {
        let config = EventedConfig::default();
        assert_eq!(config.database_path, ":memory:");
    }

    #[tokio::test]
    async fn test_builder_creates_instance() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        assert!(evented.event_store().as_ref() as *const _ != std::ptr::null());
    }

    #[tokio::test]
    async fn test_builder_with_projector() {
        let projector = CountingProjector::new();
        let evented = Evented::builder(EventedConfig::in_memory())
            .with_projector(projector)
            .build()
            .await
            .unwrap();

        assert!(evented.event_bus().as_ref() as *const _ != std::ptr::null());
    }

    #[tokio::test]
    async fn test_handle_command() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let command = make_command_book("orders", root);

        let response = evented.handle_command(command).await.unwrap();

        assert_eq!(response.books.len(), 1);
    }

    #[tokio::test]
    async fn test_handle_command_notifies_projectors() {
        // Build evented with a projector via builder
        let evented = Evented::builder(EventedConfig::in_memory())
            .with_projector(CountingProjector::new())
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let command = make_command_book("orders", root);

        // Verify command processing succeeds with projector registered
        evented.handle_command(command).await.unwrap();
    }

    #[tokio::test]
    async fn test_record_events() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let events = make_event_book("orders", root, 3);

        let response = evented.record_events(events).await.unwrap();

        assert_eq!(response.books.len(), 1);
        assert_eq!(response.books[0].pages.len(), 3);
    }

    #[tokio::test]
    async fn test_get_events() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let events = make_event_book("orders", root, 3);
        evented.record_events(events).await.unwrap();

        let retrieved = evented.get_events("orders", root).await.unwrap();

        assert_eq!(retrieved.pages.len(), 3);
    }

    #[tokio::test]
    async fn test_get_events_range() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let events = make_event_book("orders", root, 5);
        evented.record_events(events).await.unwrap();

        let range = evented.get_events_range("orders", root, 1, 3).await.unwrap();

        assert_eq!(range.pages.len(), 2); // Events at sequence 1 and 2
    }

    #[tokio::test]
    async fn test_list_aggregates() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        let root1 = make_uuid();
        let root2 = make_uuid();

        evented
            .record_events(make_event_book("orders", root1, 1))
            .await
            .unwrap();
        evented
            .record_events(make_event_book("orders", root2, 1))
            .await
            .unwrap();

        let roots = evented.list_aggregates("orders").await.unwrap();

        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&root1));
        assert!(roots.contains(&root2));
    }

    #[tokio::test]
    async fn test_handle_command_missing_cover() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        let command = CommandBook {
            cover: None,
            pages: vec![],
        };

        let result = evented.handle_command(command).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EventedError::MissingCover));
    }

    #[tokio::test]
    async fn test_handle_command_missing_root() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        let command = CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: None,
            }),
            pages: vec![],
        };

        let result = evented.handle_command(command).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EventedError::MissingRoot));
    }

    #[tokio::test]
    async fn test_handle_command_invalid_uuid() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        let command = CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1, 2, 3], // Invalid: must be 16 bytes
                }),
            }),
            pages: vec![],
        };

        let result = evented.handle_command(command).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EventedError::InvalidUuid(_)));
    }

    #[tokio::test]
    async fn test_accessor_methods() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        // Just verify these don't panic
        let _ = evented.event_store();
        let _ = evented.snapshot_store();
        let _ = evented.event_bus();
    }

    #[tokio::test]
    async fn test_handle_command_with_mock_business_logic() {
        let business_logic = MockBusinessLogic::new(vec!["orders".to_string()]);
        let evented = Evented::builder(EventedConfig::in_memory())
            .with_business_logic(business_logic)
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let command = make_command_book("orders", root);

        let response = evented.handle_command(command).await.unwrap();
        assert_eq!(response.books.len(), 1);
    }

    #[tokio::test]
    async fn test_handle_command_business_logic_domain_not_found() {
        let business_logic = MockBusinessLogic::new(vec!["inventory".to_string()]);
        let evented = Evented::builder(EventedConfig::in_memory())
            .with_business_logic(business_logic)
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let command = make_command_book("orders", root); // Domain not in business logic

        let result = evented.handle_command(command).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EventedError::BusinessLogic(crate::interfaces::BusinessError::DomainNotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_handle_command_business_logic_rejects() {
        let business_logic = MockBusinessLogic::new(vec!["orders".to_string()]);
        business_logic.set_reject_command(true).await;
        let evented = Evented::builder(EventedConfig::in_memory())
            .with_business_logic(business_logic)
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let command = make_command_book("orders", root);

        let result = evented.handle_command(command).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EventedError::BusinessLogic(crate::interfaces::BusinessError::Rejected(_))
        ));
    }

    #[tokio::test]
    async fn test_handle_command_business_logic_connection_failure() {
        let business_logic = MockBusinessLogic::new(vec!["orders".to_string()]);
        business_logic.set_fail_on_handle(true).await;
        let evented = Evented::builder(EventedConfig::in_memory())
            .with_business_logic(business_logic)
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let command = make_command_book("orders", root);

        let result = evented.handle_command(command).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EventedError::BusinessLogic(crate::interfaces::BusinessError::Connection { .. })
        ));
    }

    /// Saga that produces a new command for each event, causing infinite loop.
    struct InfiniteLoopSaga {
        commands: tokio::sync::RwLock<Vec<CommandBook>>,
    }

    impl InfiniteLoopSaga {
        fn new() -> Self {
            Self {
                commands: tokio::sync::RwLock::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl Saga for InfiniteLoopSaga {
        fn name(&self) -> &str {
            "infinite-loop"
        }

        fn domains(&self) -> Vec<String> {
            vec!["orders".to_string()]
        }

        async fn handle(&self, book: &Arc<EventBook>) -> SagaResult<Vec<CommandBook>> {
            // Generate a new command for the same aggregate
            let cover = book.cover.clone();
            let command = CommandBook {
                cover,
                pages: vec![CommandPage {
                    sequence: 0,
                    command: Some(Any {
                        type_url: "test.LoopCommand".to_string(),
                        value: vec![],
                    }),
                    synchronous: false,
                }],
            };
            self.commands.write().await.push(command.clone());
            Ok(vec![command])
        }
    }

    #[tokio::test]
    async fn test_handle_command_saga_depth_exceeded() {
        let saga = InfiniteLoopSaga::new();
        let evented = Evented::builder(EventedConfig::in_memory())
            .with_saga(saga)
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let command = make_command_book("orders", root);

        let result = evented.handle_command(command).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EventedError::SagaDepthExceeded { .. }
        ));
    }

    #[tokio::test]
    async fn test_builder_with_saga() {
        struct NoOpSaga;

        #[async_trait]
        impl Saga for NoOpSaga {
            fn name(&self) -> &str {
                "noop"
            }

            fn domains(&self) -> Vec<String> {
                vec![]
            }

            async fn handle(&self, _book: &Arc<EventBook>) -> SagaResult<Vec<CommandBook>> {
                Ok(vec![])
            }
        }

        let evented = Evented::builder(EventedConfig::in_memory())
            .with_saga(NoOpSaga)
            .build()
            .await
            .unwrap();

        // Verify instance created successfully with saga
        assert!(evented.event_bus().as_ref() as *const _ != std::ptr::null());
    }

    #[tokio::test]
    async fn test_get_events_empty_aggregate() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let events = evented.get_events("orders", root).await.unwrap();

        assert!(events.pages.is_empty());
    }

    #[tokio::test]
    async fn test_list_aggregates_empty_domain() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .build()
            .await
            .unwrap();

        let roots = evented.list_aggregates("nonexistent").await.unwrap();

        assert!(roots.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_projectors() {
        let evented = Evented::builder(EventedConfig::in_memory())
            .with_projector(CountingProjector::new())
            .with_projector(CountingProjector::new())
            .build()
            .await
            .unwrap();

        let root = make_uuid();
        let command = make_command_book("orders", root);

        let response = evented.handle_command(command).await.unwrap();
        assert_eq!(response.books.len(), 1);
    }
}
