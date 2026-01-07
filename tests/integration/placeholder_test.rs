//! Placeholder business logic integration tests.

use evented::clients::PlaceholderBusinessLogic;
use evented::interfaces::BusinessLogicClient;
use evented::proto::{
    event_page::Sequence, CommandBook, CommandPage, ContextualCommand, Cover, EventBook, EventPage,
    Uuid as ProtoUuid,
};
use prost_types::Timestamp;
use uuid::Uuid;

fn make_uuid() -> ProtoUuid {
    ProtoUuid {
        value: Uuid::new_v4().as_bytes().to_vec(),
    }
}

fn make_command(command_type: &str, sequence: u32) -> CommandPage {
    CommandPage {
        sequence,
        synchronous: false,
        command: Some(prost_types::Any {
            type_url: format!("type.googleapis.com/{}", command_type),
            value: vec![1, 2, 3],
        }),
    }
}

fn make_event(sequence: u32, event_type: &str) -> EventPage {
    EventPage {
        sequence: Some(Sequence::Num(sequence)),
        created_at: Some(Timestamp {
            seconds: 1704067200,
            nanos: 0,
        }),
        event: Some(prost_types::Any {
            type_url: format!("type.googleapis.com/{}", event_type),
            value: vec![1, 2, 3],
        }),
        synchronous: false,
    }
}

#[tokio::test]
async fn test_placeholder_creates_event_from_command() {
    let placeholder = PlaceholderBusinessLogic::with_defaults();
    let root = make_uuid();

    let cmd = ContextualCommand {
        events: Some(EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(root.clone()),
            }),
            snapshot: None,
            pages: vec![],
        }),
        command: Some(CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(root),
            }),
            pages: vec![make_command("CreateOrder", 0)],
        }),
    };

    let result = placeholder.handle("orders", cmd).await.unwrap();

    assert_eq!(result.pages.len(), 1);
    assert_eq!(result.pages[0].sequence, Some(Sequence::Num(0)));

    let event_type = result.pages[0].event.as_ref().map(|e| &e.type_url).unwrap();
    assert!(
        event_type.contains("OrderCreated"),
        "Expected OrderCreated, got {}",
        event_type
    );
}

#[tokio::test]
async fn test_placeholder_increments_sequence_from_prior_events() {
    let placeholder = PlaceholderBusinessLogic::with_defaults();
    let root = make_uuid();

    let cmd = ContextualCommand {
        events: Some(EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(root.clone()),
            }),
            snapshot: None,
            pages: vec![make_event(0, "OrderCreated"), make_event(1, "ItemAdded")],
        }),
        command: Some(CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(root),
            }),
            pages: vec![make_command("AddItem", 0)],
        }),
    };

    let result = placeholder.handle("orders", cmd).await.unwrap();

    assert_eq!(result.pages.len(), 1);
    assert_eq!(
        result.pages[0].sequence,
        Some(Sequence::Num(2)),
        "Should continue from sequence 2"
    );
}

#[tokio::test]
async fn test_placeholder_handles_multiple_commands() {
    let placeholder = PlaceholderBusinessLogic::with_defaults();
    let root = make_uuid();

    let cmd = ContextualCommand {
        events: Some(EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(root.clone()),
            }),
            snapshot: None,
            pages: vec![],
        }),
        command: Some(CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(root),
            }),
            pages: vec![
                make_command("CreateOrder", 0),
                make_command("AddItem", 1),
                make_command("AddItem", 2),
            ],
        }),
    };

    let result = placeholder.handle("orders", cmd).await.unwrap();

    assert_eq!(result.pages.len(), 3);
    assert_eq!(result.pages[0].sequence, Some(Sequence::Num(0)));
    assert_eq!(result.pages[1].sequence, Some(Sequence::Num(1)));
    assert_eq!(result.pages[2].sequence, Some(Sequence::Num(2)));
}

#[tokio::test]
async fn test_placeholder_has_domain() {
    let placeholder = PlaceholderBusinessLogic::with_defaults();

    assert!(placeholder.has_domain("orders"));
    assert!(placeholder.has_domain("inventory"));
    assert!(placeholder.has_domain("customers"));
    assert!(!placeholder.has_domain("unknown"));
}

#[tokio::test]
async fn test_placeholder_custom_domains() {
    let placeholder =
        PlaceholderBusinessLogic::new(vec!["shipping".to_string(), "billing".to_string()]);

    assert!(placeholder.has_domain("shipping"));
    assert!(placeholder.has_domain("billing"));
    assert!(!placeholder.has_domain("orders"));
}

#[tokio::test]
async fn test_placeholder_transforms_command_types() {
    let placeholder = PlaceholderBusinessLogic::with_defaults();
    let root = make_uuid();

    let test_cases = vec![
        ("CreateOrder", "OrderCreated"),
        ("AddItem", "ItemAdded"),
        ("UpdateCustomer", "CustomerUpdated"),
        ("DeleteProduct", "ProductDeleted"),
        ("CompleteOrder", "OrderCompleted"),
        ("CancelShipment", "ShipmentCancelled"),
    ];

    for (command_type, expected_event) in test_cases {
        let cmd = ContextualCommand {
            events: Some(EventBook {
                cover: Some(Cover {
                    domain: "orders".to_string(),
                    root: Some(root.clone()),
                }),
                snapshot: None,
                pages: vec![],
            }),
            command: Some(CommandBook {
                cover: Some(Cover {
                    domain: "orders".to_string(),
                    root: Some(root.clone()),
                }),
                pages: vec![make_command(command_type, 0)],
            }),
        };

        let result = placeholder.handle("orders", cmd).await.unwrap();
        let event_type = result.pages[0].event.as_ref().map(|e| &e.type_url).unwrap();

        assert!(
            event_type.contains(expected_event),
            "Command {} should produce event containing {}, got {}",
            command_type,
            expected_event,
            event_type
        );
    }
}
