//! Business logic client interface.

use async_trait::async_trait;

use crate::proto::{ContextualCommand, EventBook};

/// Result type for business logic operations.
pub type Result<T> = std::result::Result<T, BusinessError>;

/// Errors that can occur during business logic operations.
#[derive(Debug, thiserror::Error)]
pub enum BusinessError {
    #[error("Domain not found: {0}")]
    DomainNotFound(String),

    #[error("Connection failed to {domain}: {message}")]
    Connection { domain: String, message: String },

    #[error("Business logic rejected command: {0}")]
    Rejected(String),

    #[error("Timeout waiting for {domain}")]
    Timeout { domain: String },

    #[error("gRPC error: {0}")]
    Grpc(Box<tonic::Status>),
}

impl From<tonic::Status> for BusinessError {
    fn from(status: tonic::Status) -> Self {
        BusinessError::Grpc(Box::new(status))
    }
}

/// Interface for calling business logic services.
///
/// Business logic services implement the domain-specific command handling.
/// They receive a ContextualCommand (prior events + new command) and
/// return new events to be persisted.
///
/// Implementations:
/// - `StaticBusinessLogicClient` (now): Hardcoded addresses per domain
/// - `DiscoveryBusinessLogicClient` (future): Consul/K8s service discovery
#[async_trait]
pub trait BusinessLogicClient: Send + Sync {
    /// Handle a contextual command.
    ///
    /// Routes to the appropriate business logic service based on domain,
    /// sends the command, and returns the resulting events.
    async fn handle(&self, domain: &str, cmd: ContextualCommand) -> Result<EventBook>;

    /// Check if a domain is registered.
    fn has_domain(&self, domain: &str) -> bool;

    /// List all registered domains.
    fn domains(&self) -> Vec<String>;
}
