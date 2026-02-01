//! Retry utilities: backoff builders and retryable error classification.
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

/// Determines if a gRPC error is retryable (sequence conflict only).
///
/// Retryable codes:
/// - `Aborted`: Sequence conflict (concurrent write during persist)
///
/// Non-retryable:
/// - `FailedPrecondition`: client logic errors (rejected commands).
///   These map from `BusinessError::Rejected` and will never succeed on retry.
pub fn is_retryable_status(status: &Status) -> bool {
    matches!(status.code(), Code::Aborted)
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
pub async fn run_with_retry<Op>(mut operation: Op, backoff: ExponentialBuilder) -> Result<Op::Success, Op::Failure>
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
        assert!(is_retryable_status(&Status::aborted("Sequence conflict")));
        assert!(!is_retryable_status(&Status::failed_precondition(
            "Business error"
        )));
        assert!(!is_retryable_status(&Status::invalid_argument(
            "Invalid command"
        )));
        assert!(!is_retryable_status(&Status::not_found("Not found")));
        assert!(!is_retryable_status(&Status::internal("Internal error")));
    }
}
