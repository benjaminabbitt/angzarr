use super::*;
use crate::proto::{event_page, CommandPage, Cover, MergeStrategy, Uuid as ProtoUuid};
use crate::proto_ext::{calculate_set_next_seq, CommandBookExt, EventBookExt};
use prost_types::Any;

fn make_command_book(domain: &str, root: Uuid, sequence: u32) -> CommandBook {
    make_command_book_with_strategy(domain, root, sequence, MergeStrategy::MergeCommutative)
}

fn make_command_book_with_strategy(
    domain: &str,
    root: Uuid,
    sequence: u32,
    strategy: MergeStrategy,
) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![CommandPage {
            sequence,
            command: Some(Any {
                type_url: "test.Command".to_string(),
                value: vec![],
            }),
            merge_strategy: strategy as i32,
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

    let mut book = EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages,
        snapshot: None,
        ..Default::default()
    };
    calculate_set_next_seq(&mut book);
    book
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
            edition: None,
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
fn test_next_sequence_from_events() {
    let root = Uuid::new_v4();
    let events = make_event_book("orders", root, Some(4));

    assert_eq!(events.next_sequence(), 5);
}

#[test]
fn test_next_sequence_empty_events() {
    let root = Uuid::new_v4();
    let events = make_event_book("orders", root, None);

    assert_eq!(events.next_sequence(), 0);
}

#[test]
fn test_next_sequence_from_snapshot() {
    use crate::proto::{Snapshot, SnapshotRetention};

    let root = Uuid::new_v4();
    let mut events = make_event_book("orders", root, None);
    events.snapshot = Some(Snapshot {
        sequence: 10,
        state: None,
        retention: SnapshotRetention::RetentionDefault as i32,
    });
    calculate_set_next_seq(&mut events);

    assert_eq!(events.next_sequence(), 11);
}

// ============================================================================
// Merge Strategy Tests
// ============================================================================

#[test]
fn test_merge_strategy_default_is_commutative() {
    let root = Uuid::new_v4();
    let command = make_command_book("orders", root, 0);

    assert_eq!(command.merge_strategy(), MergeStrategy::MergeCommutative);
}

#[test]
fn test_merge_strategy_strict() {
    let root = Uuid::new_v4();
    let command = make_command_book_with_strategy("orders", root, 0, MergeStrategy::MergeStrict);

    assert_eq!(command.merge_strategy(), MergeStrategy::MergeStrict);
}

#[test]
fn test_merge_strategy_aggregate_handles() {
    let root = Uuid::new_v4();
    let command =
        make_command_book_with_strategy("orders", root, 0, MergeStrategy::MergeAggregateHandles);

    assert_eq!(
        command.merge_strategy(),
        MergeStrategy::MergeAggregateHandles
    );
}

#[test]
fn test_merge_strategy_empty_pages_defaults_to_commutative() {
    let command = CommandBook {
        cover: None,
        pages: vec![],
        saga_origin: None,
    };

    // Empty pages should default to Commutative
    assert_eq!(command.merge_strategy(), MergeStrategy::MergeCommutative);
}
