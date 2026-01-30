//! Retry utilities: backoff builders and retryable error classification.
//!
//! Uses `backon` for exponential backoff with jitter. Provides standard
//! backoff configurations for saga/PM command retries and gRPC connection retries.

use std::time::Duration;

use backon::ExponentialBuilder;
use tonic::{Code, Status};

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
/// - `FailedPrecondition`: Business logic errors (rejected commands).
///   These map from `BusinessError::Rejected` and will never succeed on retry.
pub fn is_retryable_status(status: &Status) -> bool {
    matches!(status.code(), Code::Aborted)
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
