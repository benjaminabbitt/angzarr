//! Sequence validation logic for AggregateService.
//!
//! Handles sequence validation and sequence computation helpers.

use prost::Message;
use tonic::Status;
use uuid::Uuid;

use crate::proto::EventBook;
use crate::storage::StorageError;

/// Result of a sequence validation check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceValidationResult {
    /// Sequence matches expected value.
    Valid,
    /// Sequence mismatch detected.
    Mismatch { expected: u32, actual: u32 },
}

/// Validates that the command sequence matches the aggregate's current sequence.
///
/// # Arguments
/// * `expected_sequence` - The sequence number from the command
/// * `actual_sequence` - The current aggregate sequence from the event store
///
/// # Returns
/// `SequenceValidationResult::Valid` if sequences match,
/// otherwise `SequenceValidationResult::Mismatch` with details.
pub fn validate_sequence(expected_sequence: u32, actual_sequence: u32) -> SequenceValidationResult {
    if expected_sequence == actual_sequence {
        SequenceValidationResult::Valid
    } else {
        SequenceValidationResult::Mismatch {
            expected: expected_sequence,
            actual: actual_sequence,
        }
    }
}

/// Creates a Status error for a sequence mismatch.
pub fn sequence_mismatch_error(expected: u32, actual: u32) -> Status {
    Status::failed_precondition(format!(
        "Sequence mismatch: command expects {}, aggregate at {}",
        expected, actual
    ))
}

/// Creates a Status error for sequence mismatch with EventBook attached as details.
///
/// The EventBook is serialized and attached to the status details,
/// allowing the caller to extract current state for retry without an extra fetch.
pub fn sequence_mismatch_error_with_state(
    expected: u32,
    actual: u32,
    current_state: &EventBook,
) -> Status {
    let message = format!(
        "Sequence mismatch: command expects {}, aggregate at {}",
        expected, actual
    );

    // Serialize EventBook to binary for status details
    let details = current_state.encode_to_vec();

    Status::with_details(
        tonic::Code::FailedPrecondition,
        message,
        details.into(),
    )
}

/// Extract EventBook from status details if present.
///
/// Returns None if details are empty or cannot be decoded.
pub fn extract_event_book_from_status(status: &Status) -> Option<EventBook> {
    let details = status.details();
    if details.is_empty() {
        return None;
    }

    EventBook::decode(details).ok()
}


/// Outcome of handling a storage error during event persistence.
#[derive(Debug)]
pub enum StorageErrorOutcome {
    /// Should abort with the given error.
    Abort(Status),
}

/// Handles storage errors during event persistence.
///
/// # Arguments
/// * `error` - The storage error that occurred
/// * `domain` - The domain name (for logging)
/// * `root_uuid` - The aggregate root UUID (for logging)
///
/// # Returns
/// `StorageErrorOutcome::Abort` with a Status error.
pub fn handle_storage_error(error: StorageError, _domain: &str, _root_uuid: Uuid) -> StorageErrorOutcome {
    match error {
        StorageError::SequenceConflict { expected, actual } => {
            StorageErrorOutcome::Abort(Status::aborted(format!(
                "Sequence conflict: expected {}, got {}",
                expected, actual
            )))
        }
        e => StorageErrorOutcome::Abort(Status::internal(format!("Failed to persist events: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_sequence_matching() {
        let result = validate_sequence(5, 5);
        assert_eq!(result, SequenceValidationResult::Valid);
    }

    #[test]
    fn test_validate_sequence_mismatch() {
        let result = validate_sequence(0, 5);
        assert_eq!(
            result,
            SequenceValidationResult::Mismatch {
                expected: 0,
                actual: 5
            }
        );
    }

    #[test]
    fn test_validate_sequence_new_aggregate() {
        let result = validate_sequence(0, 0);
        assert_eq!(result, SequenceValidationResult::Valid);
    }

    #[test]
    fn test_sequence_mismatch_error_format() {
        let status = sequence_mismatch_error(0, 5);
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
        assert!(status.message().contains("Sequence mismatch"));
        assert!(status.message().contains("0"));
        assert!(status.message().contains("5"));
    }

    #[test]
    fn test_handle_storage_error_sequence_conflict() {
        let error = StorageError::SequenceConflict {
            expected: 5,
            actual: 6,
        };
        let root = Uuid::new_v4();

        let outcome = handle_storage_error(error, "test", root);
        match outcome {
            StorageErrorOutcome::Abort(status) => {
                assert_eq!(status.code(), tonic::Code::Aborted);
                assert!(status.message().contains("Sequence conflict"));
            }
        }
    }

    #[test]
    fn test_handle_storage_error_other_error() {
        let error = StorageError::MissingCover;
        let root = Uuid::new_v4();

        let outcome = handle_storage_error(error, "test", root);
        match outcome {
            StorageErrorOutcome::Abort(status) => {
                assert_eq!(status.code(), tonic::Code::Internal);
                assert!(status.message().contains("persist events"));
            }
        }
    }
}
