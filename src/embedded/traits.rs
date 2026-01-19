//! Handler traits for embedded mode.
//!
//! Users implement these traits to provide business logic, projectors, and sagas.

use async_trait::async_trait;
use tonic::Status;

use crate::proto::{ContextualCommand, EventBook, Projection, SagaResponse};

/// Business logic handler for a domain aggregate.
///
/// Implement this trait to handle commands for a specific domain.
/// The handler receives a contextual command (command + prior events)
/// and returns new events to persist.
///
/// # Example
///
/// ```ignore
/// use angzarr::embedded::AggregateHandler;
/// use angzarr::proto::{ContextualCommand, EventBook};
///
/// struct OrdersHandler;
///
/// #[async_trait::async_trait]
/// impl AggregateHandler for OrdersHandler {
///     async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, tonic::Status> {
///         // Rebuild state from prior events
///         let state = rebuild_state(&ctx.existing);
///
///         // Validate command and produce new events
///         let events = match ctx.command.as_ref() {
///             Some(cmd) => process_command(cmd, &state)?,
///             None => return Err(tonic::Status::invalid_argument("missing command")),
///         };
///
///         Ok(events)
///     }
/// }
/// ```
#[async_trait]
pub trait AggregateHandler: Send + Sync + 'static {
    /// Handle a contextual command and return new events.
    async fn handle(&self, command: ContextualCommand) -> Result<EventBook, Status>;
}

/// Projector handler for building read models.
///
/// Implement this trait to react to events and update read models.
/// Projectors can be synchronous (blocking command response) or
/// asynchronous (running in background).
///
/// # Example
///
/// ```ignore
/// use angzarr::embedded::ProjectorHandler;
/// use angzarr::proto::{EventBook, Projection};
///
/// struct AccountingProjector {
///     db: DatabasePool,
/// }
///
/// #[async_trait::async_trait]
/// impl ProjectorHandler for AccountingProjector {
///     async fn handle(&self, events: &EventBook) -> Result<Projection, tonic::Status> {
///         for page in &events.pages {
///             // Update read model based on event
///             self.update_ledger(&page.event).await?;
///         }
///         Ok(Projection::default())
///     }
/// }
/// ```
#[async_trait]
pub trait ProjectorHandler: Send + Sync + 'static {
    /// Handle events and update read model.
    ///
    /// Returns a Projection with any data to include in command response
    /// (only used for synchronous projectors).
    async fn handle(&self, events: &EventBook) -> Result<Projection, Status>;
}

/// Saga handler for cross-aggregate workflows.
///
/// Implement this trait to orchestrate workflows across multiple aggregates.
/// Sagas receive events and can emit commands to other aggregates.
///
/// # Example
///
/// ```ignore
/// use angzarr::embedded::SagaHandler;
/// use angzarr::proto::{EventBook, SagaResponse, CommandBook};
///
/// struct FulfillmentSaga;
///
/// #[async_trait::async_trait]
/// impl SagaHandler for FulfillmentSaga {
///     async fn handle(&self, events: &EventBook) -> Result<SagaResponse, tonic::Status> {
///         let mut commands = Vec::new();
///
///         for page in &events.pages {
///             if is_order_placed(&page.event) {
///                 // Emit command to reserve inventory
///                 commands.push(create_reserve_inventory_command(&page.event));
///             }
///         }
///
///         Ok(SagaResponse {
///             commands,
///             ..Default::default()
///         })
///     }
/// }
/// ```
#[async_trait]
pub trait SagaHandler: Send + Sync + 'static {
    /// Handle events and return commands to execute.
    async fn handle(&self, events: &EventBook) -> Result<SagaResponse, Status>;
}

/// Configuration for a projector.
#[derive(Debug, Clone, Default)]
pub struct ProjectorConfig {
    /// Whether this projector should block command response.
    ///
    /// Synchronous projectors must complete before the command returns.
    /// This is useful for projectors that produce data needed by the client.
    pub synchronous: bool,

    /// Domains to subscribe to.
    ///
    /// If empty, subscribes to all domains.
    pub domains: Vec<String>,
}

impl ProjectorConfig {
    /// Create a synchronous projector config.
    pub fn sync() -> Self {
        Self {
            synchronous: true,
            ..Default::default()
        }
    }

    /// Create an asynchronous projector config.
    pub fn async_() -> Self {
        Self {
            synchronous: false,
            ..Default::default()
        }
    }

    /// Subscribe to specific domains.
    pub fn with_domains(mut self, domains: Vec<String>) -> Self {
        self.domains = domains;
        self
    }
}

/// Configuration for a saga.
#[derive(Debug, Clone, Default)]
pub struct SagaConfig {
    /// Domains to subscribe to.
    ///
    /// If empty, subscribes to all domains.
    pub domains: Vec<String>,
}

impl SagaConfig {
    /// Subscribe to specific domains.
    pub fn with_domains(mut self, domains: Vec<String>) -> Self {
        self.domains = domains;
        self
    }
}
