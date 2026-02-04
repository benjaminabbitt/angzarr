//! Client and service configuration types.
//!
//! This module contains service endpoint configuration used across
//! the system for client logic, projectors, and sagas.

use serde::Deserialize;

/// Default domain for saga compensation fallback events.
pub const DEFAULT_SAGA_FALLBACK_DOMAIN: &str = "angzarr.saga-failures";

// ============================================================================
// Configuration
// ============================================================================

/// Service endpoint configuration.
///
/// Used for all service types: client logic, projectors, and sagas.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ServiceEndpoint {
    /// Service identifier (domain for client logic, name for projectors/sagas).
    pub name: String,
    /// gRPC address (host:port).
    pub address: String,
}

/// Saga compensation configuration.
///
/// Controls how saga command rejections are handled.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SagaCompensationConfig {
    /// Domain for fallback events when client logic cannot handle revocation.
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
            fallback_domain: DEFAULT_SAGA_FALLBACK_DOMAIN.to_string(),
            dead_letter_queue_url: None,
            escalation_webhook_url: None,
            fallback_emit_system_revocation: true,
            fallback_send_to_dlq: false,
            fallback_escalate: false,
        }
    }
}

/// Process Manager configuration.
///
/// Process managers coordinate long-running workflows across multiple aggregates.
/// They maintain event-sourced state and can subscribe to multiple domains.
///
/// WARNING: Only use when saga + queries is insufficient. Consider:
/// - Can a simple saga + destination queries solve this?
/// - Is the "state" derivable from existing aggregates?
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProcessManagerConfig {
    /// Name of the process manager. Also used as the domain for PM state.
    pub name: String,
    /// gRPC address (host:port).
    pub address: String,
    /// Timeout configurations by type (e.g., "payment", "reservation").
    pub timeouts: Option<std::collections::HashMap<String, TimeoutConfig>>,
}

impl ProcessManagerConfig {
    /// Domain = name. Process manager stores its state in its own domain.
    pub fn domain(&self) -> &str {
        &self.name
    }
}

/// Timeout configuration for process manager stages.
#[derive(Debug, Clone, Deserialize)]
pub struct TimeoutConfig {
    /// Duration in minutes before timeout is triggered.
    pub duration_minutes: u32,
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
        assert_eq!(config.fallback_domain, DEFAULT_SAGA_FALLBACK_DOMAIN);
        assert!(config.dead_letter_queue_url.is_none());
        assert!(config.escalation_webhook_url.is_none());
        assert!(config.fallback_emit_system_revocation);
        assert!(!config.fallback_send_to_dlq);
        assert!(!config.fallback_escalate);
    }

    #[test]
    fn test_process_manager_config_default() {
        let config = ProcessManagerConfig::default();
        assert_eq!(config.name, "");
        assert_eq!(config.address, "");
        assert!(config.timeouts.is_none());
        assert_eq!(config.domain(), "");
    }

    #[test]
    fn test_process_manager_config_domain() {
        let config = ProcessManagerConfig {
            name: "order-fulfillment".to_string(),
            address: "localhost:50060".to_string(),
            timeouts: None,
        };
        assert_eq!(config.domain(), "order-fulfillment");
    }
}
