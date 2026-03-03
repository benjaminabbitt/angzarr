//! Tests for gRPC error classification and retry semantics.
//!
//! The retry system distinguishes between:
//! - **Retryable errors** (FailedPrecondition): Sequence mismatches where
//!   the client can fetch fresh state and retry with correct sequence.
//! - **Non-retryable errors** (Aborted, InvalidArgument, etc.): Either
//!   business rejections or errors requiring human intervention.
//!
//! Why this matters: Correct classification is critical — retrying non-retryable
//! errors wastes resources, while failing to retry transient errors causes
//! unnecessary failures.
//!
//! Key behaviors verified:
//! - FailedPrecondition is retryable (sequence mismatches with STRICT/COMMUTATIVE)
//! - Aborted is NOT retryable (MERGE_MANUAL conflicts go to DLQ)
//! - Other error codes are NOT retryable (business rejections, validation, etc.)

use super::*;

/// FailedPrecondition is retryable — client fetches fresh state and retries.
///
/// Sequence mismatches with STRICT or COMMUTATIVE merge strategy return
/// FailedPrecondition. The fix is: fetch current aggregate state, rebuild
/// command with correct sequence, and retry.
///
/// Aborted is NOT retryable — it signals MERGE_MANUAL conflicts that
/// require human review via DLQ.
#[test]
fn test_is_retryable_status() {
    // Sequence mismatch (STRICT/COMMUTATIVE) is retryable - client fetches fresh state
    assert!(is_retryable_status(&Status::failed_precondition(
        "Sequence mismatch: command expects 0, aggregate at 5"
    )));

    // ABORTED (DLQ routing with MERGE_MANUAL) is NOT retryable
    assert!(!is_retryable_status(&Status::aborted(
        "Sent to DLQ for manual review"
    )));

    // Other errors are NOT retryable
    assert!(!is_retryable_status(&Status::invalid_argument(
        "Invalid command"
    )));
    assert!(!is_retryable_status(&Status::not_found("Not found")));
    assert!(!is_retryable_status(&Status::internal("Internal error")));
}
