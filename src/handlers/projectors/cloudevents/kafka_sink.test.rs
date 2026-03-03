//! Tests for Kafka CloudEvents sink configuration.
//!
//! The Kafka sink publishes CloudEvents to Kafka topics with JSON
//! serialization. Tests verify configuration builder patterns and
//! validation of required fields.

use super::*;

// Test fixtures - not real credentials
// lgtm[rust/hardcoded-credentials]
const TEST_USER: &str = "test-user"; // codeql[rust/hard-coded-credentials]
                                     // lgtm[rust/hardcoded-credentials]
const TEST_PASSWORD: &str = "test-password"; // codeql[rust/hard-coded-credentials]

// ============================================================================
// Configuration Tests
// ============================================================================

/// Default config has sensible defaults.
#[test]
fn test_config_defaults() {
    let config = KafkaSinkConfig::default();
    assert_eq!(config.topic, "cloudevents");
    assert_eq!(config.timeout, Duration::from_secs(5));
}

/// Builder pattern configures all fields.
#[test]
fn test_config_builder() {
    // codeql[rust/hard-coded-cryptographic-value] - Test fixture, not real credentials
    let config = KafkaSinkConfig::default()
        .with_bootstrap_servers("localhost:9092".to_string())
        .with_topic("my-events".to_string())
        .with_sasl(
            TEST_USER.to_string(),
            TEST_PASSWORD.to_string(),
            "PLAIN".to_string(),
        );

    assert_eq!(config.bootstrap_servers, "localhost:9092");
    assert_eq!(config.topic, "my-events");
    assert_eq!(config.sasl_username, Some(TEST_USER.to_string()));
    assert_eq!(config.sasl_mechanism, Some("PLAIN".to_string()));
    assert_eq!(config.security_protocol, Some("SASL_SSL".to_string()));
}

/// Empty bootstrap servers fail validation.
///
/// Bootstrap servers are required - can't connect to Kafka without them.
#[test]
fn test_empty_bootstrap_servers_fails() {
    let config = KafkaSinkConfig::default();
    let result = KafkaSink::new(config);
    assert!(result.is_err());
}
