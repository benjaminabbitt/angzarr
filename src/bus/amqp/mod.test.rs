//! Tests for AMQP event bus configuration and routing.
//!
//! These are unit tests for the configuration and routing logic.
//! Integration tests (requiring RabbitMQ) are in tests/bus_amqp.rs.
//!
//! Key behaviors verified:
//! - Routing key generation from EventBook (domain + root_id)
//! - Publisher config has no queue binding
//! - Subscriber config sets routing key pattern

use super::*;
use crate::proto::{Cover, Uuid};

/// Routing key = "{domain}.{root_id_hex}".
///
/// This routing format enables topic exchange routing:
/// - "orders.*" matches all order aggregate events
/// - "#" matches all events
#[test]
fn test_routing_key_generation() {
    let book = EventBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(Uuid {
                value: b"test-123".to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
            external_id: String::new(),
        }),
        pages: vec![],
        snapshot: None,
        ..Default::default()
    };

    // "test-123" as bytes becomes "746573742d313233" in hex
    assert_eq!(AmqpEventBus::routing_key(&book), "orders.746573742d313233");
}

/// Publisher config declares exchange but no queue binding.
///
/// Publishers don't consume — they just need the exchange name.
#[test]
fn test_publisher_config() {
    let config = AmqpConfig::publisher("amqp://localhost:5672");
    assert_eq!(config.exchange, "angzarr.events");
    assert!(config.queue.is_none());
}

/// Subscriber config sets queue and routing key pattern.
///
/// Pattern "{domain}.*" routes all events for that domain to this queue.
#[test]
fn test_subscriber_config() {
    let config = AmqpConfig::subscriber("amqp://localhost:5672", "orders-projector", "orders");
    assert_eq!(config.routing_key, Some("orders.*".to_string()));
    assert_eq!(config.queue, Some("orders-projector".to_string()));
}
