//! Retry utilities: backoff builders and retryable error classification.
//!
//! DOC: This file is referenced in docs/docs/operations/error-recovery.mdx
//!      Update documentation when making changes to retry patterns.
//!
//! Uses `backon` for exponential backoff with jitter. Provides standard
//! backoff configurations for saga/PM command retries and gRPC connection retries.

use std::time::Duration;

use async_trait::async_trait;
use backon::{BackoffBuilder, ExponentialBuilder};
use tonic::{Code, Status};
use tracing::{error, warn};

/// Standard backoff for saga/PM command retries (sequence conflicts).
///
/// - Min delay: 10ms
/// - Max delay: 2s
/// - Max attempts: 10
/// - Jitter enabled
pub fn saga_backoff() -> ExponentialBuilder {
    ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(10))
        .with_max_delay(Duration::from_secs(2))
        .with_max_times(10)
        .with_jitter()
}

/// Backoff for gRPC connection retries at startup.
///
/// - Min delay: 100ms
/// - Max delay: 5s
/// - Max attempts: 30
/// - Jitter enabled
pub fn connection_backoff() -> ExponentialBuilder {
    ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(100))
        .with_max_delay(Duration::from_secs(5))
        .with_max_times(30)
        .with_jitter()
}

/// Determines if a gRPC error is retryable.
///
/// Retryable codes:
/// - `Aborted`: Storage-level sequence conflict (concurrent write race during persist).
/// - `FailedPrecondition`: Sequence mismatch (client sent stale data). Client must
///   fetch fresh state before retry — cached state is invalid.
///
/// Non-retryable:
/// - All other codes: Business rejections, network errors, server errors, etc.
///
/// Note: FAILED_PRECONDITION is retryable (sequence mismatch with STRICT/COMMUTATIVE).
/// ABORTED is NOT retryable (used for DLQ routing with MERGE_MANUAL).
///
/// Retry handlers should always fetch fresh state (not use cached) since
/// the client's view of the aggregate may be stale.
pub fn is_retryable_status(status: &Status) -> bool {
    matches!(status.code(), Code::FailedPrecondition)
}

/// The outcome of a single attempt of a retryable operation.
pub enum RetryOutcome<S, F> {
    /// The operation succeeded.
    Success(S),
    /// The operation failed with a retryable error.
    Retryable(F),
    /// The operation failed with a fatal error.
    Fatal(F),
}

/// An operation that can be retried with backoff.
#[async_trait]
pub trait RetryableOperation: Send + Sync {
    /// The output of a successful operation.
    type Success;
    /// The error type for a failed operation.
    type Failure: std::fmt::Display + Send + Sync;

    /// The name of the operation, for logging.
    fn name(&self) -> &str;

    /// Attempt to perform the operation.
    async fn try_execute(&mut self) -> RetryOutcome<Self::Success, Self::Failure>;

    /// Prepare for the next attempt after a retryable failure.
    ///
    /// This method can be used to refresh state before the next try.
    /// If it returns an error, the retry loop is aborted.
    async fn prepare_for_retry(&mut self, failure: &Self::Failure) -> Result<(), Self::Failure> {
        // Default implementation does nothing.
        let _ = failure;
        Ok(())
    }
}

/// Run a `RetryableOperation` with exponential backoff.
///
/// The operation is retried until it succeeds, fails with a fatal error,
/// or the backoff policy gives up.
pub async fn run_with_retry<Op>(
    mut operation: Op,
    backoff: ExponentialBuilder,
) -> Result<Op::Success, Op::Failure>
where
    Op: RetryableOperation,
{
    let mut attempt = 0;
    let mut delays = backoff.build();
    loop {
        attempt += 1;
        match operation.try_execute().await {
            RetryOutcome::Success(success) => return Ok(success),
            RetryOutcome::Retryable(failure) => {
                if let Some(delay) = delays.next() {
                    warn!(
                        operation = %operation.name(),
                        attempt = attempt,
                        error = %failure,
                        delay = ?delay,
                        "Operation failed, retrying after backoff"
                    );
                    if let Err(fatal_failure) = operation.prepare_for_retry(&failure).await {
                        error!(
                            operation = %operation.name(),
                            "Failed to prepare for retry: {}",
                            fatal_failure
                        );
                        return Err(fatal_failure);
                    }
                    tokio::time::sleep(delay).await;
                } else {
                    error!(
                        operation = %operation.name(),
                        attempts = attempt,
                        "Operation failed and retry limit exhausted"
                    );
                    return Err(failure);
                }
            }
            RetryOutcome::Fatal(failure) => {
                error!(
                    operation = %operation.name(),
                    attempt = attempt,
                    error = %failure,
                    "Operation failed with fatal error"
                );
                return Err(failure);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
