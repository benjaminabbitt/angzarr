//! Error types for the Angzarr client library.

use tonic::{Code, Status};

/// Result type for client operations.
pub type Result<T> = std::result::Result<T, ClientError>;

/// Errors that can occur during client operations.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Failed to establish connection to the server.
    #[error("connection failed: {0}")]
    Connection(String),

    /// Transport-level error from tonic.
    #[error("transport error: {0}")]
    Transport(#[from] tonic::transport::Error),

    /// gRPC error from the server.
    #[error("grpc error: {0}")]
    Grpc(Box<Status>),

    /// Invalid argument provided by caller.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// Failed to parse timestamp.
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),
}

impl From<Status> for ClientError {
    fn from(status: Status) -> Self {
        ClientError::Grpc(Box::new(status))
    }
}

impl ClientError {
    /// Returns the error message.
    pub fn message(&self) -> String {
        match self {
            ClientError::Connection(msg) => msg.clone(),
            ClientError::Transport(e) => e.to_string(),
            ClientError::Grpc(s) => s.message().to_string(),
            ClientError::InvalidArgument(msg) => msg.clone(),
            ClientError::InvalidTimestamp(msg) => msg.clone(),
        }
    }

    /// Returns the gRPC status code if this is a gRPC error.
    pub fn code(&self) -> Option<Code> {
        match self {
            ClientError::Grpc(s) => Some(s.code()),
            _ => None,
        }
    }

    /// Returns the underlying gRPC Status if this is a gRPC error.
    pub fn status(&self) -> Option<&Status> {
        match self {
            ClientError::Grpc(s) => Some(s),
            _ => None,
        }
    }

    /// Returns true if this is a "not found" error.
    pub fn is_not_found(&self) -> bool {
        matches!(self.code(), Some(Code::NotFound))
    }

    /// Returns true if this is a "precondition failed" error.
    pub fn is_precondition_failed(&self) -> bool {
        matches!(self.code(), Some(Code::FailedPrecondition))
    }

    /// Returns true if this is an "invalid argument" error.
    pub fn is_invalid_argument(&self) -> bool {
        matches!(self.code(), Some(Code::InvalidArgument))
            || matches!(self, ClientError::InvalidArgument(_))
    }

    /// Returns true if this is a connection or transport error.
    pub fn is_connection_error(&self) -> bool {
        matches!(self, ClientError::Connection(_) | ClientError::Transport(_))
    }
}
