use super::*;
use crate::proto::{Cover, Uuid};

#[test]
fn test_message_key_generation() {
    let book = EventBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(Uuid {
                value: b"test-123".to_vec(),
            }),
        }),
        pages: vec![],
        snapshot: None,
        correlation_id: String::new(),
        snapshot_state: None,
    };

    assert_eq!(
        KafkaEventBus::message_key(&book),
        Some("746573742d313233".to_string())
    );
}

#[test]
fn test_topic_for_domain() {
    let config = KafkaEventBusConfig::publisher("localhost:9092");
    assert_eq!(config.topic_for_domain("orders"), "angzarr.events.orders");
}

#[test]
fn test_topic_with_custom_prefix() {
    let config = KafkaEventBusConfig::publisher("localhost:9092").with_topic_prefix("myapp");
    assert_eq!(config.topic_for_domain("orders"), "myapp.events.orders");
}

#[test]
fn test_publisher_config() {
    let config = KafkaEventBusConfig::publisher("localhost:9092");
    assert_eq!(config.bootstrap_servers, "localhost:9092");
    assert!(config.group_id.is_none());
    assert!(config.domains.is_none());
}

#[test]
fn test_subscriber_config() {
    let config = KafkaEventBusConfig::subscriber(
        "localhost:9092",
        "orders-projector",
        vec!["orders".to_string()],
    );
    assert_eq!(config.group_id, Some("orders-projector".to_string()));
    assert_eq!(config.domains, Some(vec!["orders".to_string()]));
}

#[test]
fn test_sasl_config() {
    let config = KafkaEventBusConfig::publisher("localhost:9092").with_sasl(
        "user",
        "pass",
        "SCRAM-SHA-256",
    );
    assert_eq!(config.sasl_username, Some("user".to_string()));
    assert_eq!(config.sasl_password, Some("pass".to_string()));
    assert_eq!(config.sasl_mechanism, Some("SCRAM-SHA-256".to_string()));
    assert_eq!(config.security_protocol, Some("SASL_SSL".to_string()));
}

#[test]
fn test_ssl_config() {
    let config = KafkaEventBusConfig::publisher("localhost:9092")
        .with_security_protocol("SSL")
        .with_ssl_ca("/path/to/ca.crt");
    assert_eq!(config.security_protocol, Some("SSL".to_string()));
    assert_eq!(config.ssl_ca_location, Some("/path/to/ca.crt".to_string()));
}

#[test]
fn test_extract_domain() {
    let book = EventBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: None,
        }),
        pages: vec![],
        snapshot: None,
        correlation_id: String::new(),
        snapshot_state: None,
    };

    assert_eq!(KafkaEventBus::extract_domain(&book), Some("orders"));
}

#[test]
fn test_extract_domain_missing_cover() {
    let book = EventBook {
        cover: None,
        pages: vec![],
        snapshot: None,
        correlation_id: String::new(),
        snapshot_state: None,
    };

    assert_eq!(KafkaEventBus::extract_domain(&book), None);
}
