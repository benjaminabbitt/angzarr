//! Business logic clients.
//!
//! This module contains:
//! - `BusinessLogicClient` trait: Communication with business logic services
//! - Client configuration types
//! - Implementations: Static, Mock

use async_trait::async_trait;
use serde::Deserialize;

use crate::proto::{BusinessResponse, ContextualCommand};

// Implementation modules
pub mod mock;
pub mod static_client;

// Re-exports
pub use mock::MockBusinessLogic;
pub use static_client::StaticBusinessLogicClient;

// ============================================================================
// Traits
// ============================================================================

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
/// return a BusinessResponse containing either events to persist or
/// a RevocationResponse with handling instructions.
///
/// Implementations:
/// - `StaticBusinessLogicClient`: Hardcoded addresses per domain
/// - `MockBusinessLogic`: In-memory mock for testing
#[async_trait]
pub trait BusinessLogicClient: Send + Sync {
    /// Handle a contextual command.
    ///
    /// Routes to the appropriate business logic service based on domain,
    /// sends the command, and returns the BusinessResponse.
    async fn handle(&self, domain: &str, cmd: ContextualCommand) -> Result<BusinessResponse>;

    /// Check if a domain is registered.
    fn has_domain(&self, domain: &str) -> bool;

    /// List all registered domains.
    fn domains(&self) -> Vec<String>;
}

// ============================================================================
// Configuration
// ============================================================================

/// Unified service endpoint configuration.
///
/// Used for all service types: business logic, projectors, and sagas.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ServiceEndpoint {
    /// Service identifier (domain for business logic, name for projectors/sagas).
    pub name: String,
    /// gRPC address (host:port).
    pub address: String,
}

/// Business logic service endpoint (alias for backwards compatibility).
pub type BusinessLogicEndpoint = ServiceEndpoint;

/// Projector endpoint (alias for backwards compatibility).
pub type ProjectorEndpoint = ServiceEndpoint;

/// Saga endpoint (alias for backwards compatibility).
pub type SagaEndpoint = ServiceEndpoint;

/// Saga compensation configuration.
///
/// Controls how saga command rejections are handled.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SagaCompensationConfig {
    /// Domain for fallback events when business logic cannot handle revocation.
    /// Default: "angzarr.saga-failures"
    pub fallback_domain: String,
    /// Dead letter queue URL (AMQP). None = DLQ disabled.
    pub dead_letter_queue_url: Option<String>,
    /// Webhook URL for escalation alerts. None = log only.
    pub escalation_webhook_url: Option<String>,
    /// Emit SagaCompensationFailed event on fallback (empty response or gRPC error).
    pub fallback_emit_system_revocation: bool,
    /// Send to DLQ on fallback.
    pub fallback_send_to_dlq: bool,
    /// Trigger escalation on fallback.
    pub fallback_escalate: bool,
}

impl Default for SagaCompensationConfig {
    fn default() -> Self {
        Self {
            fallback_domain: "angzarr.saga-failures".to_string(),
            dead_letter_queue_url: None,
            escalation_webhook_url: None,
            fallback_emit_system_revocation: true,
            fallback_send_to_dlq: false,
            fallback_escalate: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_endpoint_default() {
        let endpoint = ServiceEndpoint::default();
        assert_eq!(endpoint.name, "");
        assert_eq!(endpoint.address, "");
    }

    #[test]
    fn test_saga_compensation_config_default() {
        let config = SagaCompensationConfig::default();
        assert_eq!(config.fallback_domain, "angzarr.saga-failures");
        assert!(config.dead_letter_queue_url.is_none());
        assert!(config.escalation_webhook_url.is_none());
        assert!(config.fallback_emit_system_revocation);
        assert!(!config.fallback_send_to_dlq);
        assert!(!config.fallback_escalate);
    }
}
