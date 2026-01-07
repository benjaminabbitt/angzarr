//! In-process saga interface.
//!
//! Sagas consume events and may produce new commands,
//! enabling cross-aggregate workflows.

use std::sync::Arc;

use async_trait::async_trait;

use crate::proto::{CommandBook, EventBook};

/// Result type for saga operations.
pub type Result<T> = std::result::Result<T, SagaError>;

/// Errors from saga operations.
#[derive(Debug, thiserror::Error)]
pub enum SagaError {
    #[error("Saga failed: {0}")]
    Failed(String),

    #[error("Command generation failed: {0}")]
    CommandFailed(String),
}

/// In-process saga interface.
///
/// Implement this trait to react to events and potentially
/// produce new commands for cross-aggregate coordination.
///
/// The `handle` method takes `&self` rather than `&mut self`.
/// Sagas that need to maintain mutable state should use
/// interior mutability (e.g., `RwLock`, `Mutex`).
///
/// # Example
///
/// ```ignore
/// struct OrderFulfillmentSaga {
///     pending_orders: RwLock<HashMap<Uuid, OrderState>>,
/// }
///
/// #[async_trait]
/// impl Saga for OrderFulfillmentSaga {
///     fn name(&self) -> &str { "order_fulfillment" }
///     fn domains(&self) -> Vec<String> { vec!["orders".into(), "inventory".into()] }
///
///     async fn handle(&self, book: &Arc<EventBook>) -> Result<Vec<CommandBook>> {
///         let mut pending = self.pending_orders.write().await;
///         let mut commands = Vec::new();
///         for event in &book.pages {
///             if is_order_created(event) {
///                 commands.push(reserve_inventory_command(event));
///             }
///         }
///         Ok(commands)
///     }
/// }
/// ```
#[async_trait]
pub trait Saga: Send + Sync {
    /// Name of this saga.
    fn name(&self) -> &str;

    /// Domains this saga is interested in.
    fn domains(&self) -> Vec<String>;

    /// Process events and optionally produce new commands.
    ///
    /// The EventBook is wrapped in Arc to allow zero-copy sharing.
    /// Use interior mutability (RwLock/Mutex) for mutable saga state.
    ///
    /// Returns a list of commands to be executed.
    /// Empty list means no follow-up actions needed.
    async fn handle(&self, book: &Arc<EventBook>) -> Result<Vec<CommandBook>>;

    /// Whether this saga requires synchronous processing.
    ///
    /// Synchronous sagas block command processing until complete
    /// and their produced commands are also processed.
    fn is_synchronous(&self) -> bool {
        false
    }
}
