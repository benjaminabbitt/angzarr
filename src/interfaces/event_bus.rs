//! Event bus interface for async delivery.

use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use tonic::Status;

use crate::proto::{EventBook, Projection};

/// Result type for bus operations.
pub type Result<T> = std::result::Result<T, BusError>;

/// Errors that can occur during bus operations.
#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Projector '{name}' failed")]
    ProjectorFailed {
        name: String,
        #[source]
        source: super::ProjectorError,
    },

    #[error("Saga '{name}' failed")]
    SagaFailed {
        name: String,
        #[source]
        source: super::SagaError,
    },

    #[error("gRPC error: {0}")]
    Grpc(#[from] Status),

    #[error("Subscribe not supported for this bus type")]
    SubscribeNotSupported,
}

/// Handler for processing events from the bus.
pub trait EventHandler: Send + Sync {
    /// Process an event book.
    fn handle(&self, book: Arc<EventBook>)
        -> BoxFuture<'static, std::result::Result<(), BusError>>;
}

/// Result of publishing events to the bus.
#[derive(Debug, Default)]
pub struct PublishResult {
    /// Projections returned by synchronous projectors.
    pub projections: Vec<Projection>,
}

/// Interface for event delivery to projectors/sagas.
///
/// Implementations:
/// - `DirectEventBus` (now): Synchronous gRPC calls to projectors
/// - `AmqpEventBus` (future): RabbitMQ via sidecar
/// - `KafkaEventBus` (future): Kafka via sidecar
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish events to consumers.
    ///
    /// The EventBook is wrapped in Arc to enforce immutability during distribution.
    /// All consumers receive a zero-copy reference to the same immutable data.
    ///
    /// For synchronous events, this blocks until all consumers acknowledge.
    /// For async events, this returns immediately after queuing.
    ///
    /// Returns projections from synchronous projectors.
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult>;

    /// Subscribe to events (for projector/saga implementations).
    ///
    /// The handler will be called for each event book received.
    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()>;
}
