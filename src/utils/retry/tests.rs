use super::*;

#[test]
fn test_default_config() {
    let config = RetryConfig::default();
    assert_eq!(config.base_delay, Duration::from_millis(10));
    assert_eq!(config.max_delay, Duration::from_secs(2));
    assert_eq!(config.max_retries, 10);
    assert!((config.jitter - 0.25).abs() < f64::EPSILON);
}

#[test]
fn test_exponential_backoff() {
    let config = RetryConfig {
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(10),
        max_retries: 5,
        jitter: 0.0, // No jitter for predictable testing
    };

    assert_eq!(config.delay_for_attempt(0), Duration::from_millis(100));
    assert_eq!(config.delay_for_attempt(1), Duration::from_millis(200));
    assert_eq!(config.delay_for_attempt(2), Duration::from_millis(400));
    assert_eq!(config.delay_for_attempt(3), Duration::from_millis(800));
    assert_eq!(config.delay_for_attempt(4), Duration::from_millis(1600));
}

#[test]
fn test_delay_capped_at_max() {
    let config = RetryConfig {
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_millis(500),
        max_retries: 10,
        jitter: 0.0,
    };

    // 100 * 2^3 = 800, should be capped at 500
    assert_eq!(config.delay_for_attempt(3), Duration::from_millis(500));
    assert_eq!(config.delay_for_attempt(10), Duration::from_millis(500));
}

#[test]
fn test_jitter_applies() {
    let config = RetryConfig {
        base_delay: Duration::from_millis(1000),
        max_delay: Duration::from_secs(10),
        max_retries: 5,
        jitter: 0.25,
    };

    let delay = config.delay_for_attempt(0);
    let ms = delay.as_millis() as f64;
    // Should be within ±25% of 1000ms
    assert!(ms >= 750.0, "Delay {} too low", ms);
    assert!(ms <= 1250.0, "Delay {} too high", ms);
}

#[test]
fn test_should_retry() {
    let config = RetryConfig {
        max_retries: 3,
        ..Default::default()
    };

    assert!(config.should_retry(0));
    assert!(config.should_retry(1));
    assert!(config.should_retry(2));
    assert!(!config.should_retry(3));
    assert!(!config.should_retry(4));
}

#[test]
fn test_is_retryable_status() {
    assert!(is_retryable_status(&Status::failed_precondition(
        "Sequence mismatch"
    )));
    assert!(is_retryable_status(&Status::aborted("Sequence conflict")));
    assert!(!is_retryable_status(&Status::invalid_argument(
        "Invalid command"
    )));
    assert!(!is_retryable_status(&Status::not_found("Not found")));
    assert!(!is_retryable_status(&Status::internal("Internal error")));
}

#[test]
fn test_no_overflow_on_large_attempt() {
    let config = RetryConfig {
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(60),
        max_retries: 100,
        jitter: 0.0,
    };

    // Should not panic on large attempt numbers
    let delay = config.delay_for_attempt(50);
    assert!(delay <= Duration::from_secs(60));
}
