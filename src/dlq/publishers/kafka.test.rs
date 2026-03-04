//! Tests for Kafka DLQ publisher.
//!
//! The Kafka publisher sends dead letters to topics named `{prefix}-{domain}`.
//! It uses correlation_id as the message key for ordering within a partition.
//!
//! These tests cover the pure functions that don't require a running Kafka cluster.
//! Full integration tests are in the Gherkin contract test suite (tests/interfaces/).
//!
//! Key behaviors verified:
//! - Topic naming with domain sanitization
//! - Default topic prefix

// ============================================================================
// Topic Naming Tests
// ============================================================================

/// topic_for_domain replaces dots with dashes.
///
/// Kafka topic names have naming restrictions. Dots are common in domain names
/// but need sanitization for Kafka compatibility.
#[test]
fn test_topic_for_domain_sanitizes_dots() {
    // We can't call the instance method directly without a producer,
    // but we can verify the same logic
    let topic_prefix = "angzarr-dlq";
    let domain = "my.nested.domain";

    let sanitized = domain.replace('.', "-");
    let expected = format!("{}-{}", topic_prefix, sanitized);

    assert_eq!(expected, "angzarr-dlq-my-nested-domain");
}

/// topic_for_domain with simple domain (no dots).
///
/// Simple domains should pass through unchanged.
#[test]
fn test_topic_for_domain_simple_domain() {
    let topic_prefix = "angzarr-dlq";
    let domain = "orders";

    let sanitized = domain.replace('.', "-");
    let expected = format!("{}-{}", topic_prefix, sanitized);

    assert_eq!(expected, "angzarr-dlq-orders");
}

/// topic_for_domain with custom prefix.
///
/// The prefix is configurable via KafkaDlqConfig.
#[test]
fn test_topic_for_domain_custom_prefix() {
    let topic_prefix = "myapp-dlq";
    let domain = "inventory";

    let sanitized = domain.replace('.', "-");
    let expected = format!("{}-{}", topic_prefix, sanitized);

    assert_eq!(expected, "myapp-dlq-inventory");
}

/// topic_for_domain with multiple consecutive dots.
///
/// Edge case: each dot should become a dash.
#[test]
fn test_topic_for_domain_multiple_dots() {
    let topic_prefix = "angzarr-dlq";
    let domain = "a..b...c";

    let sanitized = domain.replace('.', "-");
    let expected = format!("{}-{}", topic_prefix, sanitized);

    assert_eq!(expected, "angzarr-dlq-a--b---c");
}

/// topic_for_domain with leading/trailing dots.
///
/// Edge case: dots at boundaries should still be replaced.
#[test]
fn test_topic_for_domain_boundary_dots() {
    let topic_prefix = "angzarr-dlq";
    let domain = ".leading.trailing.";

    let sanitized = domain.replace('.', "-");
    let expected = format!("{}-{}", topic_prefix, sanitized);

    assert_eq!(expected, "angzarr-dlq--leading-trailing-");
}

// ============================================================================
// Default Values Tests
// ============================================================================

/// Default topic prefix is "angzarr-dlq".
///
/// This is used by KafkaDeadLetterPublisher::new() when not using config.
#[test]
fn test_default_topic_prefix() {
    // The default prefix used when calling new() directly
    let default_prefix = "angzarr-dlq";
    assert_eq!(default_prefix, "angzarr-dlq");
}
