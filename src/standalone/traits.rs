//! Handler traits for embedded mode.
//!
//! Users implement these traits to provide business logic, projectors, and sagas.

use async_trait::async_trait;
use tonic::Status;

use crate::proto::{ContextualCommand, Cover, EventBook, Projection, SagaResponse};

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

/// Saga handler for cross-aggregate workflows using two-phase protocol.
///
/// Three possible outcomes for each event:
/// 1. **Fetch destinations**: `prepare` returns covers → framework fetches state → calls `execute`
/// 2. **No fetch needed**: `prepare` returns empty → framework calls `execute` directly
/// 3. **No-op**: `execute` returns empty commands → saga doesn't act on this event
///
/// # Two-Phase Protocol
///
/// Phase 1 (`prepare`): Saga examines source events and declares what destination
/// aggregate roots it needs. Return covers for aggregates whose state you need.
/// Return empty vec if you don't need external state.
///
/// Phase 2 (`execute`): Saga receives source events plus any destination state
/// it requested, and produces commands to execute.
///
/// # Example - Simple saga (no destination state needed)
///
/// ```ignore
/// use angzarr::standalone::SagaHandler;
/// use angzarr::proto::{Cover, EventBook, SagaResponse, CommandBook};
///
/// struct FulfillmentSaga;
///
/// #[async_trait::async_trait]
/// impl SagaHandler for FulfillmentSaga {
///     async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, tonic::Status> {
///         // We don't need any destination state
///         Ok(vec![])
///     }
///
///     async fn execute(
///         &self,
///         source: &EventBook,
///         _destinations: &[EventBook],
///     ) -> Result<SagaResponse, tonic::Status> {
///         let mut commands = Vec::new();
///         for page in &source.pages {
///             if is_order_placed(&page.event) {
///                 commands.push(create_shipment_command(&page.event));
///             }
///         }
///         Ok(SagaResponse { commands, ..Default::default() })
///     }
/// }
/// ```
///
/// # Example - Saga that needs destination state
///
/// ```ignore
/// struct ReservationSaga;
///
/// #[async_trait::async_trait]
/// impl SagaHandler for ReservationSaga {
///     async fn prepare(&self, source: &EventBook) -> Result<Vec<Cover>, tonic::Status> {
///         // We need the current inventory state for each product
///         let mut covers = Vec::new();
///         for page in &source.pages {
///             if let Some(product_id) = extract_product_id(&page.event) {
///                 covers.push(Cover {
///                     domain: "inventory".to_string(),
///                     root: Some(product_id),
///                 });
///             }
///         }
///         Ok(covers)
///     }
///
///     async fn execute(
///         &self,
///         source: &EventBook,
///         destinations: &[EventBook],
///     ) -> Result<SagaResponse, tonic::Status> {
///         // Use destination state to make decisions
///         for dest in destinations {
///             let stock = compute_available_stock(dest);
///             // Generate commands based on current inventory
///         }
///         Ok(SagaResponse { commands, ..Default::default() })
///     }
/// }
/// ```
#[async_trait]
pub trait SagaHandler: Send + Sync + 'static {
    /// Phase 1: Examine source events and declare destination aggregates needed.
    ///
    /// Return covers for aggregates whose state you need before producing commands.
    /// Return empty vec if you don't need any destination state.
    async fn prepare(&self, source: &EventBook) -> Result<Vec<Cover>, Status>;

    /// Phase 2: Produce commands given source events and destination state.
    ///
    /// Called after framework fetches any destinations you declared in `prepare`.
    /// If prepare returned empty, destinations will be empty slice.
    ///
    /// Return empty commands vec if saga doesn't act on this event (no-op).
    async fn execute(
        &self,
        source: &EventBook,
        destinations: &[EventBook],
    ) -> Result<SagaResponse, Status>;
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
#[derive(Debug, Clone)]
pub struct SagaConfig {
    /// Domain to subscribe to for events.
    pub input_domain: String,
    /// Domain that saga commands must target.
    pub output_domain: String,
}

impl SagaConfig {
    /// Create a saga config with input and output domains.
    pub fn new(input_domain: impl Into<String>, output_domain: impl Into<String>) -> Self {
        Self {
            input_domain: input_domain.into(),
            output_domain: output_domain.into(),
        }
    }
}
