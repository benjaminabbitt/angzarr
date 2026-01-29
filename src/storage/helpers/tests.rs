use super::*;
use prost_types::Timestamp;

fn make_event_with_sequence(seq: Option<u32>) -> EventPage {
    EventPage {
        sequence: seq.map(Sequence::Num),
        event: None,
        created_at: None,
    }
}

fn make_event_with_force() -> EventPage {
    EventPage {
        sequence: Some(Sequence::Force(true)),
        event: None,
        created_at: None,
    }
}

#[test]
fn test_resolve_sequence_explicit_valid() {
    let event = make_event_with_sequence(Some(5));
    let mut auto = 3;
    let result = resolve_sequence(&event, 3, &mut auto).unwrap();
    assert_eq!(result, 5);
    assert_eq!(auto, 3); // unchanged
}

#[test]
fn test_resolve_sequence_explicit_conflict() {
    let event = make_event_with_sequence(Some(2));
    let mut auto = 5;
    let result = resolve_sequence(&event, 5, &mut auto);
    assert!(matches!(
        result,
        Err(StorageError::SequenceConflict {
            expected: 5,
            actual: 2
        })
    ));
}

#[test]
fn test_resolve_sequence_auto() {
    let event = make_event_with_sequence(None);
    let mut auto = 7;
    let result = resolve_sequence(&event, 5, &mut auto).unwrap();
    assert_eq!(result, 7);
    assert_eq!(auto, 8); // incremented
}

#[test]
fn test_resolve_sequence_force() {
    let event = make_event_with_force();
    let mut auto = 3;
    let result = resolve_sequence(&event, 0, &mut auto).unwrap();
    assert_eq!(result, 3);
    assert_eq!(auto, 4);
}

#[test]
fn test_parse_timestamp_present() {
    let event = EventPage {
        sequence: None,
        event: None,
        created_at: Some(Timestamp {
            seconds: 1704067200, // 2024-01-01 00:00:00 UTC
            nanos: 0,
        }),
    };
    let result = parse_timestamp(&event).unwrap();
    assert!(result.starts_with("2024-01-01"));
}

#[test]
fn test_parse_timestamp_missing_uses_now() {
    let event = EventPage {
        sequence: None,
        event: None,
        created_at: None,
    };
    let result = parse_timestamp(&event).unwrap();
    // Should be a valid RFC3339 timestamp
    assert!(result.contains('T'));
}

#[test]
fn test_parse_timestamp_invalid() {
    let event = EventPage {
        sequence: None,
        event: None,
        created_at: Some(Timestamp {
            seconds: i64::MAX,
            nanos: i32::MAX,
        }),
    };
    let result = parse_timestamp(&event);
    assert!(matches!(result, Err(StorageError::InvalidTimestamp { .. })));
}
