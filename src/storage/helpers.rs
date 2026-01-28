//! Shared storage helper functions.
//!
//! Common logic for event sequence handling and timestamp parsing
//! used across storage backend implementations.

use crate::proto::event_page::Sequence;
use crate::proto::EventPage;

use super::{Result, StorageError};

/// Resolve the sequence number for an event.
///
/// Returns the explicit sequence if provided (validating it's >= base_sequence),
/// or auto-assigns the next sequence number.
pub fn resolve_sequence(
    event: &EventPage,
    base_sequence: u32,
    auto_sequence: &mut u32,
) -> Result<u32> {
    match &event.sequence {
        Some(Sequence::Num(n)) => {
            if *n < base_sequence {
                return Err(StorageError::SequenceConflict {
                    expected: base_sequence,
                    actual: *n,
                });
            }
            Ok(*n)
        }
        Some(Sequence::Force(_)) | None => {
            let seq = *auto_sequence;
            *auto_sequence += 1;
            Ok(seq)
        }
    }
}

/// Parse event timestamp to RFC3339 string, defaulting to now.
pub fn parse_timestamp(event: &EventPage) -> Result<String> {
    match &event.created_at {
        Some(ts) => {
            let dt = chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32).ok_or(
                StorageError::InvalidTimestamp {
                    seconds: ts.seconds,
                    nanos: ts.nanos,
                },
            )?;
            Ok(dt.to_rfc3339())
        }
        None => Ok(chrono::Utc::now().to_rfc3339()),
    }
}

/// Convert a protobuf Timestamp to RFC3339 string.
pub fn timestamp_to_rfc3339(
    ts: &prost_types::Timestamp,
) -> std::result::Result<String, StorageError> {
    let dt = chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32).ok_or(
        StorageError::InvalidTimestamp {
            seconds: ts.seconds,
            nanos: ts.nanos,
        },
    )?;
    Ok(dt.to_rfc3339())
}

#[cfg(test)]
mod tests {
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
}
