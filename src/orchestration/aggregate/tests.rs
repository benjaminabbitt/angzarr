use super::*;
use crate::proto::{CommandPage, Cover, Uuid as ProtoUuid};
use prost_types::Any;

fn make_command_book(domain: &str, root: Uuid, sequence: u32) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
        }),
        pages: vec![CommandPage {
            sequence,
            command: Some(Any {
                type_url: "test.Command".to_string(),
                value: vec![],
            }),
        }],
        saga_origin: None,
    }
}

fn make_event_book(domain: &str, root: Uuid, last_sequence: Option<u32>) -> EventBook {
    use crate::proto::EventPage;

    let pages = if let Some(seq) = last_sequence {
        vec![EventPage {
            sequence: Some(event_page::Sequence::Num(seq)),
            event: Some(Any {
                type_url: "test.Event".to_string(),
                value: vec![],
            }),
            created_at: None,
        }]
    } else {
        vec![]
    };

    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
        }),
        pages,
        snapshot: None,
        snapshot_state: None,
    }
}

#[test]
fn test_parse_command_cover_valid() {
    let root = Uuid::new_v4();
    let command = make_command_book("orders", root, 0);

    let (domain, parsed_root) = parse_command_cover(&command).unwrap();

    assert_eq!(domain, "orders");
    assert_eq!(parsed_root, root);
}

#[test]
fn test_parse_command_cover_missing_cover() {
    let command = CommandBook {
        cover: None,
        pages: vec![],
        saga_origin: None,
    };

    let result = parse_command_cover(&command);
    assert!(result.is_err());
    assert!(result.unwrap_err().message().contains("cover"));
}

#[test]
fn test_parse_command_cover_missing_root() {
    let command = CommandBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: None,
            correlation_id: String::new(),
        }),
        pages: vec![],
        saga_origin: None,
    };

    let result = parse_command_cover(&command);
    assert!(result.is_err());
    assert!(result.unwrap_err().message().contains("root"));
}

#[test]
fn test_extract_command_sequence() {
    let root = Uuid::new_v4();
    let command = make_command_book("orders", root, 5);

    assert_eq!(extract_command_sequence(&command), 5);
}

#[test]
fn test_extract_command_sequence_empty_pages() {
    let command = CommandBook {
        cover: None,
        pages: vec![],
        saga_origin: None,
    };

    assert_eq!(extract_command_sequence(&command), 0);
}

#[test]
fn test_compute_next_sequence_from_events() {
    let root = Uuid::new_v4();
    let events = make_event_book("orders", root, Some(4));

    assert_eq!(compute_next_sequence(&events), 5);
}

#[test]
fn test_compute_next_sequence_empty_events() {
    let root = Uuid::new_v4();
    let events = make_event_book("orders", root, None);

    assert_eq!(compute_next_sequence(&events), 0);
}

#[test]
fn test_compute_next_sequence_from_snapshot() {
    use crate::proto::Snapshot;

    let root = Uuid::new_v4();
    let mut events = make_event_book("orders", root, None);
    events.snapshot = Some(Snapshot {
        sequence: 10,
        state: None,
    });

    assert_eq!(compute_next_sequence(&events), 11);
}
