//! Client traits for gateway and query operations.
//!
//! These traits define the client API for interacting with angzarr services.
//! Both standalone (in-process) and distributed (gRPC) modes implement
//! the same traits, enabling deploy-anywhere client code.

use async_trait::async_trait;
use tonic::{Code, Status};

use crate::proto::{
    CommandBook, CommandResponse, EventBook, ProcessManagerHandleResponse, Projection, Query,
    SagaResponse, SpeculateAggregateRequest, SpeculatePmRequest, SpeculateProjectorRequest,
    SpeculateSagaRequest,
};

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

/// Trait for gateway client operations (command execution).
///
/// Implement this trait to create mock clients for testing or
/// alternative transport implementations.
#[async_trait]
pub trait GatewayClient: Send + Sync {
    /// Execute a command asynchronously (fire and forget).
    async fn execute(&self, command: CommandBook) -> Result<CommandResponse>;
}

/// Trait for speculative execution operations.
///
/// Supports "what-if" scenarios: executing commands, projectors, sagas,
/// and process managers without persisting side effects.
#[async_trait]
pub trait SpeculativeClient: Send + Sync {
    /// Execute a command speculatively (no persistence).
    async fn aggregate(&self, request: SpeculateAggregateRequest) -> Result<CommandResponse>;

    /// Speculatively execute a projector against events.
    async fn projector(&self, request: SpeculateProjectorRequest) -> Result<Projection>;

    /// Speculatively execute a saga against events.
    async fn saga(&self, request: SpeculateSagaRequest) -> Result<SagaResponse>;

    /// Speculatively execute a process manager against events.
    async fn process_manager(
        &self,
        request: SpeculatePmRequest,
    ) -> Result<ProcessManagerHandleResponse>;
}

/// Trait for event query client operations.
///
/// Implement this trait to create mock clients for testing or
/// alternative transport implementations.
#[async_trait]
pub trait QueryClient: Send + Sync {
    /// Get events for the given query.
    async fn get_events(&self, query: Query) -> Result<EventBook>;
}
