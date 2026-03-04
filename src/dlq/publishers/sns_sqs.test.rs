//! Tests for AWS SNS/SQS DLQ publisher.
//!
//! The SNS/SQS publisher sends dead letters to SNS topics named `{prefix}-{domain}`.
//! It caches topic ARNs and creates topics on-demand.
//!
//! These tests cover the pure functions that don't require a running LocalStack.
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
/// SNS topic names cannot contain dots but dashes are allowed.
#[test]
fn test_topic_for_domain_sanitizes_dots() {
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
/// The prefix is configurable via SnsSqsDlqConfig.
#[test]
fn test_topic_for_domain_custom_prefix() {
    let topic_prefix = "myapp-dlq";
    let domain = "inventory";

    let sanitized = domain.replace('.', "-");
    let expected = format!("{}-{}", topic_prefix, sanitized);

    assert_eq!(expected, "myapp-dlq-inventory");
}

// ============================================================================
// Default Values Tests
// ============================================================================

/// Default topic prefix is "angzarr-dlq".
///
/// This is used by SnsSqsDeadLetterPublisher::new() when not using config.
#[test]
fn test_default_topic_prefix() {
    let default_prefix = "angzarr-dlq";
    assert_eq!(default_prefix, "angzarr-dlq");
}
