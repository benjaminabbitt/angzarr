use super::*;
use crate::proto::{
    command_page, event_page, CommandPage, Cover, MergeStrategy, Uuid as ProtoUuid,
};
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
            external_id: String::new(),
        }),
        pages: vec![CommandPage {
            sequence,
            payload: Some(command_page::Payload::Command(Any {
                type_url: "test.Command".to_string(),
                value: vec![],
            })),
            merge_strategy: strategy as i32,
        }],
        saga_origin: None,
    }
}

fn make_event_book(domain: &str, root: Uuid, last_sequence: Option<u32>) -> EventBook {
    use crate::proto::EventPage;

    let pages = if let Some(seq) = last_sequence {
        vec![EventPage {
            sequence_type: Some(event_page::SequenceType::Sequence(seq)),
            payload: Some(event_page::Payload::Event(Any {
                type_url: "test.Event".to_string(),
                value: vec![],
            })),
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
            external_id: String::new(),
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
            external_id: String::new(),
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

// ============================================================================
// Commutative Merge Helper Tests - Catch mutations in field extraction
// ============================================================================

#[test]
fn test_extract_command_fields_field_a() {
    let root = Uuid::new_v4();
    let command = CommandBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
            external_id: String::new(),
        }),
        pages: vec![CommandPage {
            sequence: 0,
            payload: Some(command_page::Payload::Command(Any {
                type_url: "test.UpdateFieldA".to_string(),
                value: vec![],
            })),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
        }],
        saga_origin: None,
    };

    let fields = extract_command_fields(&command);
    assert!(fields.contains("field_a"));
    assert!(!fields.contains("field_b"));
    assert!(!fields.contains("*"));
}

#[test]
fn test_extract_command_fields_field_b() {
    let root = Uuid::new_v4();
    let command = CommandBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
            external_id: String::new(),
        }),
        pages: vec![CommandPage {
            sequence: 0,
            payload: Some(command_page::Payload::Command(Any {
                type_url: "test.UpdateFieldB".to_string(),
                value: vec![],
            })),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
        }],
        saga_origin: None,
    };

    let fields = extract_command_fields(&command);
    assert!(!fields.contains("field_a"));
    assert!(fields.contains("field_b"));
    assert!(!fields.contains("*"));
}

#[test]
fn test_extract_command_fields_update_both() {
    let root = Uuid::new_v4();
    let command = CommandBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
            external_id: String::new(),
        }),
        pages: vec![CommandPage {
            sequence: 0,
            payload: Some(command_page::Payload::Command(Any {
                type_url: "test.UpdateBoth".to_string(),
                value: vec![],
            })),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
        }],
        saga_origin: None,
    };

    let fields = extract_command_fields(&command);
    assert!(fields.contains("field_a"));
    assert!(fields.contains("field_b"));
    assert!(!fields.contains("*"));
}

#[test]
fn test_extract_command_fields_unknown_returns_wildcard() {
    let root = Uuid::new_v4();
    let command = CommandBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
            external_id: String::new(),
        }),
        pages: vec![CommandPage {
            sequence: 0,
            payload: Some(command_page::Payload::Command(Any {
                type_url: "test.UnknownCommand".to_string(),
                value: vec![],
            })),
            merge_strategy: MergeStrategy::MergeCommutative as i32,
        }],
        saga_origin: None,
    };

    let fields = extract_command_fields(&command);
    assert!(
        fields.contains("*"),
        "unknown command should return wildcard"
    );
}

#[test]
fn test_extract_command_fields_empty_pages() {
    let command = CommandBook {
        cover: None,
        pages: vec![],
        saga_origin: None,
    };

    let fields = extract_command_fields(&command);
    assert!(fields.is_empty(), "empty pages should return empty set");
}

// ============================================================================
// Event Filtering Tests
// ============================================================================

#[test]
fn test_build_events_up_to_sequence_filters_correctly() {
    let root = Uuid::new_v4();
    let events = EventBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
            external_id: String::new(),
        }),
        pages: vec![
            crate::proto::EventPage {
                sequence_type: Some(event_page::SequenceType::Sequence(0)),
                payload: None,
                created_at: None,
            },
            crate::proto::EventPage {
                sequence_type: Some(event_page::SequenceType::Sequence(1)),
                payload: None,
                created_at: None,
            },
            crate::proto::EventPage {
                sequence_type: Some(event_page::SequenceType::Sequence(2)),
                payload: None,
                created_at: None,
            },
            crate::proto::EventPage {
                sequence_type: Some(event_page::SequenceType::Sequence(3)),
                payload: None,
                created_at: None,
            },
        ],
        snapshot: None,
        next_sequence: 4,
    };

    let filtered = build_events_up_to_sequence(&events, 2);
    assert_eq!(filtered.pages.len(), 2, "should have events 0 and 1");
    assert_eq!(filtered.next_sequence, 2);
}

#[test]
fn test_build_events_up_to_sequence_zero_returns_empty() {
    let root = Uuid::new_v4();
    let events = EventBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
            external_id: String::new(),
        }),
        pages: vec![crate::proto::EventPage {
            sequence_type: Some(event_page::SequenceType::Sequence(0)),
            payload: None,
            created_at: None,
        }],
        snapshot: None,
        next_sequence: 1,
    };

    let filtered = build_events_up_to_sequence(&events, 0);
    assert!(filtered.pages.is_empty(), "up_to 0 should return empty");
}

// ============================================================================
// Test State Parsing - Catch mutations in diff logic
// ============================================================================

#[test]
fn test_parse_test_state_fields_simple() {
    let fields = parse_test_state_fields(r#"{"field_a":100,"field_b":"hello"}"#);
    assert_eq!(fields.get("field_a"), Some(&"100".to_string()));
    assert_eq!(fields.get("field_b"), Some(&"\"hello\"".to_string()));
}

#[test]
fn test_parse_test_state_fields_empty() {
    let fields = parse_test_state_fields("{}");
    assert!(fields.is_empty());
}

#[test]
fn test_diff_test_state_fields_identical() {
    let before = r#"{"field_a":100}"#.as_bytes();
    let after = r#"{"field_a":100}"#.as_bytes();

    let changed = diff_test_state_fields(before, after);
    assert!(
        changed.is_empty(),
        "identical states should have no changes"
    );
}

#[test]
fn test_diff_test_state_fields_single_change() {
    let before = r#"{"field_a":100,"field_b":200}"#.as_bytes();
    let after = r#"{"field_a":100,"field_b":300}"#.as_bytes();

    let changed = diff_test_state_fields(before, after);
    assert_eq!(changed.len(), 1);
    assert!(changed.contains("field_b"));
    assert!(!changed.contains("field_a"));
}

#[test]
fn test_diff_test_state_fields_multiple_changes() {
    let before = r#"{"field_a":100,"field_b":200}"#.as_bytes();
    let after = r#"{"field_a":999,"field_b":888}"#.as_bytes();

    let changed = diff_test_state_fields(before, after);
    assert_eq!(changed.len(), 2);
    assert!(changed.contains("field_a"));
    assert!(changed.contains("field_b"));
}

#[test]
fn test_diff_test_state_fields_field_added() {
    let before = r#"{"field_a":100}"#.as_bytes();
    let after = r#"{"field_a":100,"field_b":200}"#.as_bytes();

    let changed = diff_test_state_fields(before, after);
    assert!(changed.contains("field_b"), "new field should be detected");
}

#[test]
fn test_diff_test_state_fields_field_removed() {
    let before = r#"{"field_a":100,"field_b":200}"#.as_bytes();
    let after = r#"{"field_a":100}"#.as_bytes();

    let changed = diff_test_state_fields(before, after);
    assert!(
        changed.contains("field_b"),
        "removed field should be detected"
    );
}

// ============================================================================
// Type URL Diff Tests - Catch mutations in diff_state_fields
// ============================================================================

#[test]
fn test_diff_state_fields_different_types() {
    let before = Any {
        type_url: "type.a".to_string(),
        value: vec![1, 2, 3],
    };
    let after = Any {
        type_url: "type.b".to_string(),
        value: vec![1, 2, 3],
    };

    let changed = diff_state_fields(&before, &after);
    assert!(
        changed.contains("*"),
        "different types should return wildcard"
    );
}

#[test]
fn test_diff_state_fields_same_bytes() {
    let before = Any {
        type_url: "test.Unknown".to_string(),
        value: vec![1, 2, 3],
    };
    let after = Any {
        type_url: "test.Unknown".to_string(),
        value: vec![1, 2, 3],
    };

    let changed = diff_state_fields(&before, &after);
    assert!(
        changed.is_empty(),
        "identical bytes should return empty set"
    );
}

#[test]
fn test_diff_state_fields_different_bytes_unknown_type() {
    let before = Any {
        type_url: "test.Unknown".to_string(),
        value: vec![1, 2, 3],
    };
    let after = Any {
        type_url: "test.Unknown".to_string(),
        value: vec![4, 5, 6],
    };

    let changed = diff_state_fields(&before, &after);
    assert!(
        changed.contains("*"),
        "different bytes with unknown type should return wildcard"
    );
}

// ============================================================================
// Fact Pipeline Parsing Tests
// ============================================================================

#[test]
fn test_parse_event_cover_valid() {
    let root = Uuid::new_v4();
    let event = make_event_book("inventory", root, Some(0));

    let (domain, parsed_root) = parse_event_cover(&event).unwrap();

    assert_eq!(domain, "inventory");
    assert_eq!(parsed_root, root);
}

#[test]
fn test_parse_event_cover_missing_cover() {
    let event = EventBook {
        cover: None,
        pages: vec![],
        snapshot: None,
        next_sequence: 0,
    };

    let result = parse_event_cover(&event);
    assert!(result.is_err());
}

#[test]
fn test_parse_event_cover_missing_root() {
    let event = EventBook {
        cover: Some(Cover {
            domain: "test".to_string(),
            root: None,
            correlation_id: String::new(),
            edition: None,
            external_id: String::new(),
        }),
        pages: vec![],
        snapshot: None,
        next_sequence: 0,
    };

    let result = parse_event_cover(&event);
    assert!(result.is_err());
}
