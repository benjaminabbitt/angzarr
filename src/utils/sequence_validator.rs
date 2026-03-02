//! Sequence validation logic for AggregateService.
//!
//! DOC: This file is referenced in docs/docs/operations/error-recovery.mdx
//!      Update documentation when making changes to sequence validation.
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
///
/// Uses `FailedPrecondition` because sequence mismatches are client errors —
/// the client sent a command with stale sequence information. The client
/// must fetch fresh state before retrying. This is NOT automatically retryable.
///
/// `Aborted` is reserved for storage-level conflicts (concurrent write races)
/// which ARE retryable since the client had correct information at validation time.
pub fn sequence_mismatch_error(expected: u32, actual: u32) -> Status {
    Status::failed_precondition(format!(
        "Sequence mismatch: command expects {}, aggregate at {}",
        expected, actual
    ))
}

/// Creates a Status error for sequence mismatch with EventBook attached as details.
///
/// The EventBook is serialized and attached to the status details,
/// allowing the caller to extract current state for a manual retry.
///
/// Uses `FailedPrecondition` — this is a client error, not automatically retryable.
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

    Status::with_details(tonic::Code::FailedPrecondition, message, details.into())
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
pub fn handle_storage_error(
    error: StorageError,
    _domain: &str,
    _root_uuid: Uuid,
) -> StorageErrorOutcome {
    match error {
        StorageError::SequenceConflict { expected, actual } => {
            StorageErrorOutcome::Abort(Status::failed_precondition(format!(
                "Sequence conflict: expected {}, got {}",
                expected, actual
            )))
        }
        e => StorageErrorOutcome::Abort(Status::internal(format!("Failed to persist events: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    //! Tests for sequence validation and optimistic concurrency control.
    //!
    //! Sequence numbers are the foundation of event sourcing consistency:
    //! - Commands carry expected sequence (client's view of aggregate state)
    //! - Aggregates have actual sequence (true current state)
    //! - Mismatch = stale client → reject with FailedPrecondition
    //!
    //! These tests verify:
    //! - Matching sequences pass validation
    //! - Mismatches are detected and reported correctly
    //! - Error messages include both expected and actual values for debugging
    //! - Storage errors are classified correctly for retry vs abort decisions

    use super::*;

    // ============================================================================
    // Sequence Validation Tests
    // ============================================================================

    /// Matching sequences validate successfully.
    ///
    /// When command sequence equals aggregate sequence, the client has current
    /// state and the command can proceed.
    #[test]
    fn test_validate_sequence_matching() {
        let result = validate_sequence(5, 5);
        assert_eq!(result, SequenceValidationResult::Valid);
    }

    /// Sequence mismatch detected and reported with both values.
    ///
    /// When command expects sequence 0 but aggregate is at 5, the client has
    /// stale state. The mismatch result includes both values for error reporting.
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

    /// New aggregate creation: both sequences at zero is valid.
    ///
    /// Creating a new aggregate means command sequence 0 and aggregate
    /// sequence 0 (non-existent). This is the bootstrap case.
    #[test]
    fn test_validate_sequence_new_aggregate() {
        let result = validate_sequence(0, 0);
        assert_eq!(result, SequenceValidationResult::Valid);
    }

    // ============================================================================
    // Error Formatting Tests
    // ============================================================================

    /// Sequence mismatch error uses FailedPrecondition and includes both values.
    ///
    /// FailedPrecondition indicates a client error — the client sent stale data.
    /// The message includes both sequence numbers for debugging.
    #[test]
    fn test_sequence_mismatch_error_format() {
        let status = sequence_mismatch_error(0, 5);
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
        assert!(status.message().contains("Sequence mismatch"));
        assert!(status.message().contains("0"));
        assert!(status.message().contains("5"));
    }

    // ============================================================================
    // Storage Error Handling Tests
    // ============================================================================

    /// Sequence conflict from storage returns FailedPrecondition (retryable).
    ///
    /// When concurrent writers race and storage rejects due to sequence
    /// conflict, the error maps to FailedPrecondition. This allows the
    /// retry system to fetch fresh state and try again.
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
                // Sequence conflicts are retryable (FAILED_PRECONDITION)
                assert_eq!(status.code(), tonic::Code::FailedPrecondition);
                assert!(status.message().contains("Sequence conflict"));
            }
        }
    }

    /// Non-conflict storage errors return Internal (non-retryable).
    ///
    /// Errors like MissingCover indicate structural problems that won't be
    /// fixed by retry. These map to Internal status for immediate abort.
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

    // ============================================================================
    // EventBook Status Details Tests
    // ============================================================================
    //
    // When sequence mismatches occur, the current aggregate state can be
    // attached to the error status. This allows sophisticated clients to
    // extract the state and retry without an additional fetch round-trip.

    /// Sequence mismatch error with state attaches EventBook as details.
    ///
    /// The EventBook is serialized to binary and attached to the status.
    /// Clients can extract it to see current aggregate state for retry.
    #[test]
    fn test_sequence_mismatch_error_with_state_roundtrip() {
        use crate::proto::{Cover, EventPage, Uuid as ProtoUuid};

        let event_book = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1, 2, 3, 4],
                }),
                correlation_id: "corr-123".to_string(),
                edition: None,
                external_id: String::new(),
            }),
            pages: vec![EventPage {
                sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(5)),
                created_at: None,
                payload: None,
            }],
            snapshot: None,
            ..Default::default()
        };

        let status = sequence_mismatch_error_with_state(0, 5, &event_book);

        // Verify status is correct
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
        assert!(status.message().contains("0"));
        assert!(status.message().contains("5"));

        // Verify EventBook can be extracted
        let extracted = extract_event_book_from_status(&status);
        assert!(extracted.is_some());

        let extracted = extracted.unwrap();
        assert_eq!(extracted.cover.unwrap().domain, "orders");
        assert_eq!(extracted.pages.len(), 1);
    }

    /// Empty status details returns None.
    ///
    /// Not all errors include EventBook details. The extract function
    /// must handle this gracefully.
    #[test]
    fn test_extract_event_book_from_status_empty_details() {
        let status = sequence_mismatch_error(0, 5);
        let extracted = extract_event_book_from_status(&status);
        assert!(extracted.is_none());
    }

    /// Invalid details bytes returns None (not panic).
    ///
    /// Malformed details should be handled gracefully, not crash.
    #[test]
    fn test_extract_event_book_from_status_invalid_bytes() {
        let status = Status::with_details(
            tonic::Code::FailedPrecondition,
            "test",
            vec![0xFF, 0xFF, 0xFF].into(),
        );
        let extracted = extract_event_book_from_status(&status);
        assert!(extracted.is_none());
    }
}
