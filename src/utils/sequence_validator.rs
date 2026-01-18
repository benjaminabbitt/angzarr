//! Sequence validation logic for AggregateService.
//!
//! Handles sequence validation, auto-resequence conflict handling,
//! and sequence computation helpers.

use tonic::Status;
use tracing::warn;
use uuid::Uuid;

use crate::storage::StorageError;

/// Maximum number of retries for auto_resequence on sequence conflicts.
pub const MAX_RESEQUENCE_RETRIES: u32 = 3;

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
/// When `auto_resequence` is enabled, this validation is skipped as write-time
/// validation will handle conflicts through retry logic.
///
/// # Arguments
/// * `expected_sequence` - The sequence number from the command
/// * `actual_sequence` - The current aggregate sequence from the event store
/// * `auto_resequence` - Whether auto-resequence is enabled
///
/// # Returns
/// `SequenceValidationResult::Valid` if sequences match or auto_resequence is enabled,
/// otherwise `SequenceValidationResult::Mismatch` with details.
pub fn validate_sequence(
    expected_sequence: u32,
    actual_sequence: u32,
    auto_resequence: bool,
) -> SequenceValidationResult {
    if auto_resequence {
        // Skip pre-validation when auto_resequence is enabled
        // Write-time validation handles conflicts instead
        return SequenceValidationResult::Valid;
    }

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

/// Outcome of handling a storage error during event persistence.
#[derive(Debug)]
pub enum StorageErrorOutcome {
    /// Should retry the operation with fresh state.
    Retry,
    /// Should abort with the given error.
    Abort(Status),
}

/// Handles storage errors during event persistence, implementing retry logic
/// for sequence conflicts when auto_resequence is enabled.
///
/// # Arguments
/// * `error` - The storage error that occurred
/// * `domain` - The domain name (for logging)
/// * `root_uuid` - The aggregate root UUID (for logging)
/// * `attempt` - Current attempt number (1-based)
/// * `auto_resequence` - Whether auto-resequence is enabled
///
/// # Returns
/// `StorageErrorOutcome::Retry` if the operation should be retried,
/// `StorageErrorOutcome::Abort` with a Status error otherwise.
pub fn handle_storage_error(
    error: StorageError,
    domain: &str,
    root_uuid: Uuid,
    attempt: u32,
    auto_resequence: bool,
) -> StorageErrorOutcome {
    match error {
        StorageError::SequenceConflict { expected, actual } => {
            if auto_resequence && attempt < MAX_RESEQUENCE_RETRIES {
                warn!(
                    domain = %domain,
                    root = %root_uuid,
                    attempt = attempt,
                    expected = expected,
                    actual = actual,
                    "Sequence conflict, retrying with fresh state"
                );
                StorageErrorOutcome::Retry
            } else if auto_resequence {
                StorageErrorOutcome::Abort(Status::aborted(format!(
                    "Sequence conflict after {} retries: expected {}, got {}",
                    MAX_RESEQUENCE_RETRIES, expected, actual
                )))
            } else {
                StorageErrorOutcome::Abort(Status::aborted(format!(
                    "Sequence conflict: expected {}, got {} (auto_resequence disabled)",
                    expected, actual
                )))
            }
        }
        e => StorageErrorOutcome::Abort(Status::internal(format!("Failed to persist events: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_sequence_matching() {
        let result = validate_sequence(5, 5, false);
        assert_eq!(result, SequenceValidationResult::Valid);
    }

    #[test]
    fn test_validate_sequence_mismatch() {
        let result = validate_sequence(0, 5, false);
        assert_eq!(
            result,
            SequenceValidationResult::Mismatch {
                expected: 0,
                actual: 5
            }
        );
    }

    #[test]
    fn test_validate_sequence_auto_resequence_skips_validation() {
        // Even with mismatch, auto_resequence returns Valid
        let result = validate_sequence(0, 5, true);
        assert_eq!(result, SequenceValidationResult::Valid);
    }

    #[test]
    fn test_validate_sequence_new_aggregate() {
        let result = validate_sequence(0, 0, false);
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
    fn test_handle_storage_error_sequence_conflict_retry() {
        let error = StorageError::SequenceConflict {
            expected: 5,
            actual: 6,
        };
        let root = Uuid::new_v4();

        let outcome = handle_storage_error(error, "test", root, 1, true);
        assert!(matches!(outcome, StorageErrorOutcome::Retry));
    }

    #[test]
    fn test_handle_storage_error_sequence_conflict_max_retries() {
        let error = StorageError::SequenceConflict {
            expected: 5,
            actual: 6,
        };
        let root = Uuid::new_v4();

        let outcome = handle_storage_error(error, "test", root, MAX_RESEQUENCE_RETRIES, true);
        match outcome {
            StorageErrorOutcome::Abort(status) => {
                assert_eq!(status.code(), tonic::Code::Aborted);
                assert!(status.message().contains("retries"));
            }
            StorageErrorOutcome::Retry => panic!("Expected Abort, got Retry"),
        }
    }

    #[test]
    fn test_handle_storage_error_sequence_conflict_no_auto_resequence() {
        let error = StorageError::SequenceConflict {
            expected: 5,
            actual: 6,
        };
        let root = Uuid::new_v4();

        let outcome = handle_storage_error(error, "test", root, 1, false);
        match outcome {
            StorageErrorOutcome::Abort(status) => {
                assert_eq!(status.code(), tonic::Code::Aborted);
                assert!(status.message().contains("auto_resequence disabled"));
            }
            StorageErrorOutcome::Retry => panic!("Expected Abort, got Retry"),
        }
    }

    #[test]
    fn test_handle_storage_error_other_error() {
        let error = StorageError::MissingCover;
        let root = Uuid::new_v4();

        let outcome = handle_storage_error(error, "test", root, 1, true);
        match outcome {
            StorageErrorOutcome::Abort(status) => {
                assert_eq!(status.code(), tonic::Code::Internal);
                assert!(status.message().contains("persist events"));
            }
            StorageErrorOutcome::Retry => panic!("Expected Abort, got Retry"),
        }
    }
}
