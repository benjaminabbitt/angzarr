//! Tests for HTTP CloudEvents sink configuration.
//!
//! The HTTP sink POSTs CloudEvents batches to webhook endpoints with retry.
//! Tests verify configuration builder patterns and retry status code
//! classification.

use super::*;

// ============================================================================
// Configuration Tests
// ============================================================================

/// Default config has sensible defaults.
#[test]
fn test_config_defaults() {
    let config = HttpSinkConfig::default();
    assert_eq!(config.timeout, Duration::from_secs(30));
    assert_eq!(config.batch_size, 100);
    assert!(config.headers.is_empty());
}

/// Builder pattern configures all fields.
#[test]
fn test_config_builder() {
    let config = HttpSinkConfig::default()
        .with_endpoint("https://example.com/events".to_string())
        .with_timeout(Duration::from_secs(60))
        .with_batch_size(50)
        .with_header("Authorization".to_string(), "Bearer token".to_string());

    assert_eq!(config.endpoint, "https://example.com/events");
    assert_eq!(config.timeout, Duration::from_secs(60));
    assert_eq!(config.batch_size, 50);
    assert_eq!(config.headers.len(), 1);
}

/// Empty endpoint fails validation.
///
/// Endpoint URL is required - can't POST without a destination.
#[test]
fn test_empty_endpoint_fails() {
    let config = HttpSinkConfig::default();
    let result = HttpSink::new(config);
    assert!(result.is_err());
}

// ============================================================================
// Retry Classification Tests
// ============================================================================

/// 429 and 5xx status codes are retryable.
///
/// Rate limiting (429) and server errors (5xx) are typically transient.
/// Client errors (4xx except 429) indicate permanent failure.
#[test]
fn test_retryable_status_codes() {
    use reqwest::StatusCode;

    assert!(HttpSink::is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
    assert!(HttpSink::is_retryable_status(
        StatusCode::INTERNAL_SERVER_ERROR
    ));
    assert!(HttpSink::is_retryable_status(StatusCode::BAD_GATEWAY));
    assert!(HttpSink::is_retryable_status(
        StatusCode::SERVICE_UNAVAILABLE
    ));

    assert!(!HttpSink::is_retryable_status(StatusCode::BAD_REQUEST));
    assert!(!HttpSink::is_retryable_status(StatusCode::UNAUTHORIZED));
    assert!(!HttpSink::is_retryable_status(StatusCode::NOT_FOUND));
}
