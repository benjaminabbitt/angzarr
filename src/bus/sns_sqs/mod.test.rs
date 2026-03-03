//! Tests for AWS SNS/SQS event bus configuration.
//!
//! These are unit tests for topic/queue naming and configuration.
//! Integration tests (requiring LocalStack) are in tests/bus_sns_sqs.rs.
//!
//! Key behaviors verified:
//! - FIFO topic naming (required for ordering by message_group_id)
//! - FIFO queue naming (must match FIFO topics)
//! - Publisher config has no subscription
//! - Subscriber config sets domains and subscription ID
//! - Custom endpoint/region for LocalStack testing

use super::*;

/// Topics use FIFO suffix for message ordering.
///
/// FIFO topics enable message_group_id (aggregate root ID) ordering,
/// ensuring events for the same aggregate are delivered in sequence.
#[test]
fn test_topic_for_domain() {
    let config = SnsSqsConfig::publisher();
    // FIFO topics require .fifo suffix
    assert_eq!(
        config.topic_for_domain("orders"),
        "angzarr-events-orders.fifo"
    );
    assert_eq!(
        config.topic_for_domain("game.player0"),
        "angzarr-events-game-player0.fifo"
    );
}

/// Custom topic prefix for multi-tenant/namespaced deployments.
#[test]
fn test_topic_with_custom_prefix() {
    let config = SnsSqsConfig::publisher().with_topic_prefix("myapp");
    assert_eq!(
        config.topic_for_domain("orders"),
        "myapp-events-orders.fifo"
    );
}

/// Queue names include subscription ID for consumer group isolation.
///
/// Multiple consumers can subscribe to the same domain with different
/// subscription IDs, each getting their own queue.
#[test]
fn test_queue_for_domain() {
    let config = SnsSqsConfig::subscriber("saga-fulfillment", vec![]);
    // FIFO queues require .fifo suffix (to match FIFO topics)
    assert_eq!(
        config.queue_for_domain("orders"),
        "angzarr-saga-fulfillment-orders.fifo"
    );
}

/// Publisher config has no subscription details.
#[test]
fn test_publisher_config() {
    let config = SnsSqsConfig::publisher();
    assert!(config.subscription_id.is_none());
    assert!(config.domains.is_empty());
}

/// Subscriber config captures subscription ID and domains.
#[test]
fn test_subscriber_config() {
    let config = SnsSqsConfig::subscriber("orders-projector", vec!["orders".to_string()]);
    assert_eq!(config.subscription_id, Some("orders-projector".to_string()));
    assert_eq!(config.domains, vec!["orders".to_string()]);
}

/// Custom endpoint for LocalStack testing.
#[test]
fn test_endpoint_config() {
    let config = SnsSqsConfig::publisher()
        .with_region("us-west-2")
        .with_endpoint("http://localhost:4566");
    assert_eq!(config.region, Some("us-west-2".to_string()));
    assert_eq!(
        config.endpoint_url,
        Some("http://localhost:4566".to_string())
    );
}
