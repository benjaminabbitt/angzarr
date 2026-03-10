//! Error types for gap-fill operations.

use crate::storage::StorageError;

/// Errors that can occur during gap-fill operations.
#[derive(Debug, thiserror::Error)]
pub enum GapFillError {
    /// Storage error when fetching checkpoint or events.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// EventBook is missing required cover information.
    #[error("EventBook missing cover")]
    MissingCover,

    /// EventBook cover is missing root UUID.
    #[error("EventBook cover missing root")]
    MissingRoot,

    /// EventBook cover is missing edition.
    #[error("EventBook cover missing edition")]
    MissingEdition,

    /// Transport error when connecting to remote service.
    #[error("Transport error: {0}")]
    Transport(String),

    /// gRPC error when fetching events.
    #[error("gRPC error: {0}")]
    Grpc(String),
}

/// Result type for gap-fill operations.
pub type Result<T> = std::result::Result<T, GapFillError>;
