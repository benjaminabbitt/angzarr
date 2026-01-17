//! Business logic, projector, and saga configuration types.

use serde::Deserialize;

/// Business logic service endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct BusinessLogicEndpoint {
    /// Domain this service handles.
    pub domain: String,
    /// gRPC address.
    pub address: String,
}

/// Projector endpoint.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProjectorEndpoint {
    /// Name of the projector.
    pub name: String,
    /// gRPC address.
    pub address: String,
    /// If true, wait for response before continuing.
    pub synchronous: bool,
}

/// Saga endpoint.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SagaEndpoint {
    /// Name of the saga.
    pub name: String,
    /// gRPC address.
    pub address: String,
    /// If true, wait for response before continuing.
    pub synchronous: bool,
}

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
    fn test_projector_endpoint_default() {
        let endpoint = ProjectorEndpoint::default();
        assert_eq!(endpoint.name, "");
        assert_eq!(endpoint.address, "");
        assert!(!endpoint.synchronous);
    }

    #[test]
    fn test_saga_endpoint_default() {
        let endpoint = SagaEndpoint::default();
        assert_eq!(endpoint.name, "");
        assert_eq!(endpoint.address, "");
        assert!(!endpoint.synchronous);
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
