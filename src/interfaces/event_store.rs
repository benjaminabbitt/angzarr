//! Event storage interface.

use async_trait::async_trait;
use uuid::Uuid;

use crate::proto::EventPage;

/// Result type for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;

/// Errors that can occur during storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Event not found: domain={domain}, root={root}")]
    NotFound { domain: String, root: Uuid },

    #[error("Sequence conflict: expected {expected}, got {actual}")]
    SequenceConflict { expected: u32, actual: u32 },

    #[error("Invalid timestamp: seconds={seconds}, nanos={nanos}")]
    InvalidTimestamp { seconds: i64, nanos: i32 },

    #[error("Invalid UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Protobuf decode error: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),

    #[error("Cover missing from EventBook")]
    MissingCover,

    #[error("Root UUID missing from Cover")]
    MissingRoot,

    #[error("MongoDB error: {0}")]
    Mongo(#[from] mongodb::error::Error),

    #[error("EventStoreDB error: {0}")]
    EventStoreDb(String),
}

/// Interface for event persistence.
///
/// Implementations:
/// - `MongoEventStore`: MongoDB storage
/// - `PostgresEventStore`: PostgreSQL storage
/// - `EventStoreDbEventStore`: EventStoreDB storage
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Store events for an aggregate root.
    ///
    /// Events are appended to the existing event stream for this root.
    /// Sequence numbers are validated for consistency.
    async fn add(&self, domain: &str, root: Uuid, events: Vec<EventPage>) -> Result<()>;

    /// Retrieve all events for an aggregate.
    async fn get(&self, domain: &str, root: Uuid) -> Result<Vec<EventPage>>;

    /// Retrieve events from sequence N onwards.
    async fn get_from(&self, domain: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>>;

    /// Retrieve events in range [from, to).
    async fn get_from_to(
        &self,
        domain: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>>;

    /// List all aggregate roots in a domain.
    async fn list_roots(&self, domain: &str) -> Result<Vec<Uuid>>;

    /// List all domains that have events stored.
    async fn list_domains(&self) -> Result<Vec<String>>;

    /// Get the next sequence number for an aggregate.
    async fn get_next_sequence(&self, domain: &str, root: Uuid) -> Result<u32>;
}
