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

// ============================================================================
// Backoff Builder Tests
// ============================================================================

/// saga_backoff() configures appropriate limits for command retries.
///
/// - Fast initial retry (10ms) for quick sequence conflict resolution
/// - Cap at 2s to avoid long blocking
/// - 10 attempts for typical contention scenarios
#[test]
fn test_saga_backoff_configuration() {
    use backon::BackoffBuilder;

    let builder = saga_backoff();
    let backoff = builder.build();

    // Should produce exactly 10 delays (max_times = 10)
    let delays: Vec<_> = backoff.collect();
    assert_eq!(
        delays.len(),
        10,
        "Should have exactly 10 delays (max_times=10), got {}",
        delays.len()
    );

    // First delay should be small (around 10ms, jitter can double it)
    let first = delays[0];
    assert!(
        first >= Duration::from_millis(5) && first < Duration::from_millis(100),
        "First delay should be around 10ms, got {:?}",
        first
    );
}

/// connection_backoff() configures appropriate limits for startup retries.
///
/// - Moderate initial retry (100ms) for network operations
/// - Cap at 5s for K8s service discovery
/// - 30 attempts for ~2-3 minutes total retry time
#[test]
fn test_connection_backoff_configuration() {
    use backon::BackoffBuilder;

    let builder = connection_backoff();
    let backoff = builder.build();

    // Should produce exactly 30 delays (max_times = 30)
    let delays: Vec<_> = backoff.collect();
    assert_eq!(
        delays.len(),
        30,
        "Should have exactly 30 delays (max_times=30), got {}",
        delays.len()
    );

    // First delay should be around 100ms (jitter can vary it)
    let first = delays[0];
    assert!(
        first >= Duration::from_millis(50) && first < Duration::from_millis(500),
        "First delay should be around 100ms, got {:?}",
        first
    );
}

// ============================================================================
// RetryOutcome Tests
// ============================================================================

/// RetryOutcome::Success carries the success value.
#[test]
fn test_retry_outcome_success() {
    let outcome: RetryOutcome<i32, String> = RetryOutcome::Success(42);
    match outcome {
        RetryOutcome::Success(val) => assert_eq!(val, 42),
        _ => panic!("Expected Success"),
    }
}

/// RetryOutcome::Retryable carries the failure value.
#[test]
fn test_retry_outcome_retryable() {
    let outcome: RetryOutcome<i32, String> = RetryOutcome::Retryable("temp error".to_string());
    match outcome {
        RetryOutcome::Retryable(err) => assert_eq!(err, "temp error"),
        _ => panic!("Expected Retryable"),
    }
}

/// RetryOutcome::Fatal carries the failure value.
#[test]
fn test_retry_outcome_fatal() {
    let outcome: RetryOutcome<i32, String> = RetryOutcome::Fatal("permanent error".to_string());
    match outcome {
        RetryOutcome::Fatal(err) => assert_eq!(err, "permanent error"),
        _ => panic!("Expected Fatal"),
    }
}
