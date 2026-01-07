//! In-process projector interface.
//!
//! Projectors consume events and build read models/projections.

use std::sync::Arc;

use async_trait::async_trait;

use crate::proto::{EventBook, Projection};

/// Result type for projector operations.
pub type Result<T> = std::result::Result<T, ProjectorError>;

/// Errors from projector operations.
#[derive(Debug, thiserror::Error)]
pub enum ProjectorError {
    #[error("Projection failed: {0}")]
    Failed(String),

    #[error("Storage error: {0}")]
    Storage(String),
}

/// In-process projector interface.
///
/// Implement this trait to handle events and build projections
/// without requiring external gRPC services.
///
/// The `project` method takes `&self` rather than `&mut self`.
/// Projectors that need to maintain mutable state should use
/// interior mutability (e.g., `RwLock`, `Mutex`).
///
/// # Example
///
/// ```ignore
/// struct OrderSummaryProjector {
///     summaries: RwLock<HashMap<Uuid, OrderSummary>>,
/// }
///
/// #[async_trait]
/// impl Projector for OrderSummaryProjector {
///     fn name(&self) -> &str { "order_summary" }
///     fn domains(&self) -> Vec<String> { vec!["orders".into()] }
///
///     async fn project(&self, book: &Arc<EventBook>) -> Result<Option<Projection>> {
///         let mut summaries = self.summaries.write().await;
///         for event in &book.pages {
///             // Update read model based on event
///         }
///         Ok(None)
///     }
/// }
/// ```
#[async_trait]
pub trait Projector: Send + Sync {
    /// Name of this projector.
    fn name(&self) -> &str;

    /// Domains this projector is interested in.
    fn domains(&self) -> Vec<String>;

    /// Process events and update projection.
    ///
    /// The EventBook is wrapped in Arc to allow zero-copy sharing.
    /// Use interior mutability (RwLock/Mutex) for mutable projector state.
    ///
    /// Returns `Some(Projection)` for synchronous projectors that
    /// need to return data to the caller.
    async fn project(&self, book: &Arc<EventBook>) -> Result<Option<Projection>>;

    /// Whether this projector requires synchronous processing.
    ///
    /// Synchronous projectors block command processing until complete.
    fn is_synchronous(&self) -> bool {
        false
    }
}
