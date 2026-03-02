//! Tests for shared storage helper functions.
//!
//! These helpers are used by all storage backends to ensure consistent
//! behavior for sequence handling, timestamp parsing, and EventBook assembly.
//!
//! Key behaviors verified:
//! - Sequence validation (optimistic concurrency control)
//! - Timestamp parsing (RFC3339 format for event ordering)
//! - Sequence extraction from EventPage

use super::*;
use crate::proto::event_page;
use prost_types::Timestamp;

fn make_event_with_sequence(seq: u32) -> EventPage {
    EventPage {
        sequence_type: Some(event_page::SequenceType::Sequence(seq)),
        payload: None,
        created_at: None,
    }
}

// ============================================================================
// Sequence Resolution Tests
// ============================================================================

/// Valid sequence >= base_sequence is accepted.
///
/// This is the happy path: client provides correct sequence matching
/// their view of aggregate state.
#[test]
fn test_resolve_sequence_explicit_valid() {
    let event = make_event_with_sequence(5);
    let mut auto = 3;
    let result = resolve_sequence(&event, 3, &mut auto).unwrap();
    assert_eq!(result, 5);
}

/// Sequence < base_sequence triggers SequenceConflict error.
///
/// Optimistic concurrency: if event's sequence is below what we expect,
/// another writer updated the aggregate. Client must refetch and retry.
#[test]
fn test_resolve_sequence_explicit_conflict() {
    let event = make_event_with_sequence(2);
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

/// Sequence 0 is valid for new aggregates.
///
/// First event always has sequence 0. Verifies zero doesn't trigger
/// any off-by-one edge cases.
#[test]
fn test_resolve_sequence_zero() {
    let event = make_event_with_sequence(0);
    let mut auto = 0;
    let result = resolve_sequence(&event, 0, &mut auto).unwrap();
    assert_eq!(result, 0);
}

// ============================================================================
// Timestamp Parsing Tests
// ============================================================================

/// Protobuf Timestamp is converted to RFC3339 string.
///
/// RFC3339 is used for storage and querying. The conversion must
/// preserve the exact time value.
#[test]
fn test_parse_timestamp_present() {
    let event = EventPage {
        sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(0)),
        payload: None,
        created_at: Some(Timestamp {
            seconds: 1704067200, // 2024-01-01 00:00:00 UTC
            nanos: 0,
        }),
    };
    let result = parse_timestamp(&event).unwrap();
    assert!(result.starts_with("2024-01-01"));
}

/// Missing timestamp defaults to current time.
///
/// Events should have timestamps, but if omitted, use now() rather
/// than failing. This makes the API more forgiving for simple cases.
#[test]
fn test_parse_timestamp_missing_uses_now() {
    let event = EventPage {
        sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(0)),
        payload: None,
        created_at: None,
    };
    let result = parse_timestamp(&event).unwrap();
    // Should be a valid RFC3339 timestamp
    assert!(result.contains('T'));
}

/// Invalid timestamp values return error.
///
/// Extreme values (i64::MAX seconds) can't be converted to DateTime.
/// Fail explicitly rather than silently truncating or wrapping.
#[test]
fn test_parse_timestamp_invalid() {
    let event = EventPage {
        sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(0)),
        payload: None,
        created_at: Some(Timestamp {
            seconds: i64::MAX,
            nanos: i32::MAX,
        }),
    };
    let result = parse_timestamp(&event);
    assert!(matches!(result, Err(StorageError::InvalidTimestamp { .. })));
}

// ============================================================================
// Sequence Extraction Tests
// ============================================================================

/// event_sequence extracts sequence number from EventPage.
///
/// Helper function used by storage backends to get sequence for
/// ordering and conflict detection.
#[test]
fn test_event_sequence() {
    let event = make_event_with_sequence(42);
    assert_eq!(event_sequence(&event), 42);
}
