//! Tests for standalone handler traits and configuration.
//!
//! These tests verify:
//! - Config builders produce correct defaults and chain properly
//! - Pure helper functions (issuer extraction, revocation responses) work correctly
//! - Data structures implement required traits (Debug, Clone, Default)
//!
//! The handlers themselves (CommandHandler, SagaHandler, etc.) are trait definitions
//! with default implementations tested via integration tests.

use super::traits::*;
use crate::proto::{CommandBook, EventBook, Notification, RejectionNotification};
use prost::Message;

// ============================================================================
// ProjectorConfig Tests
// ============================================================================

#[test]
fn test_projector_config_sync_creates_synchronous_config() {
    let config = ProjectorConfig::sync();

    assert!(config.synchronous);
    assert!(config.domains.is_empty());
}

#[test]
fn test_projector_config_async_creates_asynchronous_config() {
    let config = ProjectorConfig::async_();

    assert!(!config.synchronous);
    assert!(config.domains.is_empty());
}

#[test]
fn test_projector_config_with_domains() {
    let config =
        ProjectorConfig::sync().with_domains(vec!["orders".to_string(), "inventory".to_string()]);

    assert!(config.synchronous);
    assert_eq!(config.domains, vec!["orders", "inventory"]);
}

#[test]
fn test_projector_config_default_is_async() {
    let config = ProjectorConfig::default();

    assert!(!config.synchronous);
    assert!(config.domains.is_empty());
}

// ============================================================================
// SagaConfig Tests
// ============================================================================

#[test]
fn test_saga_config_new_creates_config_with_single_output() {
    let config = SagaConfig::new("orders", "fulfillment");

    assert_eq!(config.input_domain, "orders");
    assert_eq!(config.output_domains, vec!["fulfillment"]);
}

#[test]
fn test_saga_config_with_output_adds_domain() {
    let config = SagaConfig::new("orders", "fulfillment").with_output("inventory");

    assert_eq!(config.input_domain, "orders");
    assert_eq!(config.output_domains, vec!["fulfillment", "inventory"]);
}

#[test]
fn test_saga_config_multiple_outputs() {
    let config = SagaConfig::new("orders", "fulfillment")
        .with_output("inventory")
        .with_output("notification");

    assert_eq!(config.output_domains.len(), 3);
    assert!(config.output_domains.contains(&"fulfillment".to_string()));
    assert!(config.output_domains.contains(&"inventory".to_string()));
    assert!(config.output_domains.contains(&"notification".to_string()));
}

// ============================================================================
// ProcessManagerConfig Tests
// ============================================================================

#[test]
fn test_pm_config_new_creates_config_with_empty_subscriptions() {
    let config = ProcessManagerConfig::new("order-fulfillment-pm");

    assert_eq!(config.domain, "order-fulfillment-pm");
    assert!(config.subscriptions.is_empty());
}

#[test]
fn test_pm_config_with_subscriptions() {
    let targets = vec![
        crate::descriptor::Target {
            domain: "orders".to_string(),
            types: vec!["OrderPlaced".to_string()],
        },
        crate::descriptor::Target {
            domain: "inventory".to_string(),
            types: vec![],
        },
    ];

    let config = ProcessManagerConfig::new("my-pm").with_subscriptions(targets.clone());

    assert_eq!(config.domain, "my-pm");
    assert_eq!(config.subscriptions.len(), 2);
    assert_eq!(config.subscriptions[0].domain, "orders");
    assert_eq!(config.subscriptions[1].domain, "inventory");
}

// ============================================================================
// FactContext Tests
// ============================================================================

#[test]
fn test_fact_context_debug() {
    let ctx = FactContext {
        facts: EventBook::default(),
        prior_events: None,
    };

    // Verify Debug trait works
    let debug_str = format!("{:?}", ctx);
    assert!(debug_str.contains("FactContext"));
}

#[test]
fn test_fact_context_clone() {
    let ctx = FactContext {
        facts: EventBook::default(),
        prior_events: Some(EventBook::default()),
    };

    let cloned = ctx.clone();
    assert!(cloned.prior_events.is_some());
}

// ============================================================================
// ProcessManagerHandleResult Tests
// ============================================================================

#[test]
fn test_pm_handle_result_default() {
    let result = ProcessManagerHandleResult::default();

    assert!(result.commands.is_empty());
    assert!(result.process_events.is_none());
    assert!(result.facts.is_empty());
}

#[test]
fn test_pm_handle_result_debug() {
    let result = ProcessManagerHandleResult {
        commands: vec![CommandBook::default()],
        process_events: Some(EventBook::default()),
        facts: vec![EventBook::default()],
    };

    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("ProcessManagerHandleResult"));
}

// ============================================================================
// extract_issuer_name Tests
// ============================================================================

#[test]
fn test_extract_issuer_name_with_valid_notification() {
    let rejection = RejectionNotification {
        issuer_name: "orders".to_string(),
        rejection_reason: "insufficient stock".to_string(),
        ..Default::default()
    };

    let notification = Notification {
        payload: Some(prost_types::Any {
            type_url: "angzarr.RejectionNotification".to_string(),
            value: rejection.encode_to_vec(),
        }),
        ..Default::default()
    };

    let issuer = extract_issuer_name(&notification);
    assert_eq!(issuer, "orders");
}

#[test]
fn test_extract_issuer_name_with_no_payload() {
    let notification = Notification {
        payload: None,
        ..Default::default()
    };

    let issuer = extract_issuer_name(&notification);
    assert_eq!(issuer, "unknown");
}

#[test]
fn test_extract_issuer_name_with_invalid_payload() {
    let notification = Notification {
        payload: Some(prost_types::Any {
            type_url: "some.OtherType".to_string(),
            value: vec![0xFF, 0xFF, 0xFF], // Invalid protobuf
        }),
        ..Default::default()
    };

    let issuer = extract_issuer_name(&notification);
    assert_eq!(issuer, "unknown");
}

// ============================================================================
// build_command_handler_revocation_response Tests
// ============================================================================

#[test]
fn test_build_command_handler_revocation_response() {
    let response = build_command_handler_revocation_response("fulfillment");

    assert!(response.emit_system_revocation);
    assert!(response.reason.contains("CommandHandler"));
    assert!(response.reason.contains("fulfillment"));
}

// ============================================================================
// build_pm_revocation_response Tests
// ============================================================================

#[test]
fn test_build_pm_revocation_response() {
    let response = build_pm_revocation_response("inventory");

    assert!(response.emit_system_revocation);
    assert!(response.reason.contains("ProcessManager"));
    assert!(response.reason.contains("inventory"));
}
