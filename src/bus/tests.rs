use super::*;
use crate::proto::{event_page::Sequence, Cover, EventPage, Target, Uuid as ProtoUuid};
use prost_types::Any;

fn make_event_book(domain: &str, event_types: &[&str]) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: uuid::Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: "test-correlation".to_string(),
            edition: None,
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
        ..Default::default()
    }
}

#[test]
fn test_messaging_config_default() {
    let config = MessagingConfig::default();
    assert_eq!(config.messaging_type, MessagingType::Amqp);
    assert_eq!(config.amqp.url, "amqp://localhost:5672");
}

#[test]
fn test_target_matches_domain_only() {
    let book = make_event_book("order", &["OrderCreated"]);
    let target = Target {
        domain: "order".to_string(),
        types: vec![],
    };
    assert!(target_matches(&book, &target));
}

#[test]
fn test_target_matches_wrong_domain() {
    let book = make_event_book("order", &["OrderCreated"]);
    let target = Target {
        domain: "inventory".to_string(),
        types: vec![],
    };
    assert!(!target_matches(&book, &target));
}

#[test]
fn test_target_matches_specific_event_type() {
    let book = make_event_book("order", &["OrderCreated", "OrderShipped"]);
    let target = Target {
        domain: "order".to_string(),
        types: vec!["OrderCreated".to_string()],
    };
    assert!(target_matches(&book, &target));
}

#[test]
fn test_target_matches_event_type_not_present() {
    let book = make_event_book("order", &["OrderCreated"]);
    let target = Target {
        domain: "order".to_string(),
        types: vec!["OrderShipped".to_string()],
    };
    assert!(!target_matches(&book, &target));
}

#[test]
fn test_any_target_matches_first() {
    let book = make_event_book("order", &["OrderCreated"]);
    let targets = vec![
        Target {
            domain: "order".to_string(),
            types: vec!["OrderCreated".to_string()],
        },
        Target {
            domain: "inventory".to_string(),
            types: vec![],
        },
    ];
    assert!(any_target_matches(&book, &targets));
}

#[test]
fn test_any_target_matches_second() {
    let book = make_event_book("inventory", &["StockReserved"]);
    let targets = vec![
        Target {
            domain: "order".to_string(),
            types: vec![],
        },
        Target {
            domain: "inventory".to_string(),
            types: vec![],
        },
    ];
    assert!(any_target_matches(&book, &targets));
}

#[test]
fn test_any_target_matches_none() {
    let book = make_event_book("customer", &["CustomerCreated"]);
    let targets = vec![
        Target {
            domain: "order".to_string(),
            types: vec![],
        },
        Target {
            domain: "inventory".to_string(),
            types: vec![],
        },
    ];
    assert!(!any_target_matches(&book, &targets));
}
