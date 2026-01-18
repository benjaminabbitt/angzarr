//! Client configuration types.
//!
//! This module contains service endpoint configuration used across
//! the system for business logic, projectors, and sagas.

use serde::Deserialize;

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

/// Business logic service endpoint.
pub type BusinessLogicEndpoint = ServiceEndpoint;

/// Projector endpoint.
pub type ProjectorEndpoint = ServiceEndpoint;

/// Saga endpoint.
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
