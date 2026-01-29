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
mod tests;
