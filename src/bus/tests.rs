use super::*;
use crate::proto::{event_page::Sequence, Cover, EventPage, Uuid as ProtoUuid};
use prost_types::Any;

fn make_event_book(domain: &str, event_types: &[&str]) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: uuid::Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: "test-correlation".to_string(),
        }),
        pages: event_types
            .iter()
            .enumerate()
            .map(|(i, et)| EventPage {
                sequence: Some(Sequence::Num(i as u32)),
                created_at: None,
                event: Some(Any {
                    type_url: format!("type.googleapis.com/example.{}", et),
                    value: vec![],
                }),
            })
            .collect(),
        snapshot: None,
        snapshot_state: None,
    }
}

#[test]
fn test_messaging_config_default() {
    let config = MessagingConfig::default();
    assert_eq!(config.messaging_type, MessagingType::Amqp);
    assert_eq!(config.amqp.url, "amqp://localhost:5672");
}

#[test]
fn test_subscription_matches_domain_only() {
    let book = make_event_book("order", &["OrderCreated"]);
    let sub = Subscription {
        domain: "order".to_string(),
        event_types: vec![], // Empty = all events
    };
    assert!(subscription_matches(&book, &sub));
}

#[test]
fn test_subscription_matches_wrong_domain() {
    let book = make_event_book("order", &["OrderCreated"]);
    let sub = Subscription {
        domain: "inventory".to_string(),
        event_types: vec![],
    };
    assert!(!subscription_matches(&book, &sub));
}

#[test]
fn test_subscription_matches_specific_event_type() {
    let book = make_event_book("order", &["OrderCreated", "OrderShipped"]);
    let sub = Subscription {
        domain: "order".to_string(),
        event_types: vec!["OrderCreated".to_string()],
    };
    assert!(subscription_matches(&book, &sub));
}

#[test]
fn test_subscription_matches_event_type_not_present() {
    let book = make_event_book("order", &["OrderCreated"]);
    let sub = Subscription {
        domain: "order".to_string(),
        event_types: vec!["OrderShipped".to_string()],
    };
    assert!(!subscription_matches(&book, &sub));
}

#[test]
fn test_any_subscription_matches_first() {
    let book = make_event_book("order", &["OrderCreated"]);
    let subs = vec![
        Subscription {
            domain: "order".to_string(),
            event_types: vec!["OrderCreated".to_string()],
        },
        Subscription {
            domain: "inventory".to_string(),
            event_types: vec![],
        },
    ];
    assert!(any_subscription_matches(&book, &subs));
}

#[test]
fn test_any_subscription_matches_second() {
    let book = make_event_book("inventory", &["StockReserved"]);
    let subs = vec![
        Subscription {
            domain: "order".to_string(),
            event_types: vec![],
        },
        Subscription {
            domain: "inventory".to_string(),
            event_types: vec![],
        },
    ];
    assert!(any_subscription_matches(&book, &subs));
}

#[test]
fn test_any_subscription_matches_none() {
    let book = make_event_book("customer", &["CustomerCreated"]);
    let subs = vec![
        Subscription {
            domain: "order".to_string(),
            event_types: vec![],
        },
        Subscription {
            domain: "inventory".to_string(),
            event_types: vec![],
        },
    ];
    assert!(!any_subscription_matches(&book, &subs));
}
