//! Tests for Google Pub/Sub event bus configuration.
//!
//! These are unit tests for topic/subscription naming and configuration.
//! Integration tests (requiring Pub/Sub emulator) are in tests/bus_pubsub.rs.
//!
//! Key behaviors verified:
//! - Topic naming with domain sanitization (dots to dashes)
//! - Subscription naming with subscription ID
//! - Custom topic prefix support

use super::*;

/// Topic name sanitizes dots to dashes for Pub/Sub compatibility.
///
/// Pub/Sub topic names can't contain dots, so "game.player0" becomes
/// "game-player0" in the topic name.
#[test]
fn test_topic_for_domain() {
    let config = PubSubConfig::publisher("my-project");
    assert_eq!(config.topic_for_domain("orders"), "angzarr-events-orders");
    assert_eq!(
        config.topic_for_domain("game.player0"),
        "angzarr-events-game-player0"
    );
}

/// Custom topic prefix for multi-tenant deployments.
#[test]
fn test_topic_with_custom_prefix() {
    let config = PubSubConfig::publisher("my-project").with_topic_prefix("myapp");
    assert_eq!(config.topic_for_domain("orders"), "myapp-events-orders");
}

/// Subscription name includes subscription ID for consumer isolation.
///
/// Multiple consumers can subscribe to the same topic with different
/// subscription IDs, each receiving their own copy of messages.
#[test]
fn test_subscription_for_domain() {
    let config = PubSubConfig::subscriber("my-project", "saga-fulfillment", vec![]);
    assert_eq!(
        config.subscription_for_domain("orders"),
        "angzarr-saga-fulfillment-orders"
    );
}
