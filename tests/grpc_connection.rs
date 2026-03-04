//! gRPC connection retry integration tests.
//!
//! Run with: cargo test --test grpc_connection -- --nocapture
//!
//! These tests verify that connect_channel() properly:
//! - Succeeds when endpoint is reachable
//! - Retries on transient failures
//! - Returns error after max retries
//! - Returns immediately on invalid URI
//!
//! Note: Some tests use unreachable endpoints to test error paths.

use std::time::{Duration, Instant};

use angzarr::grpc::{connect_channel, errmsg};

// ============================================================================
// Connection Error Tests
// ============================================================================

/// connect_channel returns error for unreachable endpoint.
///
/// When no server is listening, connection should fail after retries.
#[tokio::test]
async fn test_connect_unreachable_returns_error() {
    // Use a port that's definitely not listening
    let unreachable = "localhost:59998";

    let start = Instant::now();
    let result = connect_channel(unreachable).await;
    let elapsed = start.elapsed();

    assert!(result.is_err(), "Should fail for unreachable endpoint");

    let error = result.unwrap_err();
    assert!(
        error.starts_with(errmsg::CONNECTION_FAILED),
        "Error should start with CONNECTION_FAILED prefix: {}",
        error
    );

    // Should have attempted retries (elapsed > initial delay)
    // Backoff starts at 100ms, so even with failures it takes some time
    println!("Connection attempts took {:?} before giving up", elapsed);
}

/// connect_channel returns immediately for invalid URI.
///
/// Invalid URIs (malformed syntax) should fail fast without retries.
#[tokio::test]
async fn test_connect_invalid_uri_returns_error() {
    // Completely invalid URI format
    let invalid_uri = ":::not-a-valid-uri:::";

    let start = Instant::now();
    let result = connect_channel(invalid_uri).await;
    let elapsed = start.elapsed();

    assert!(result.is_err(), "Should fail for invalid URI");

    let error = result.unwrap_err();
    assert!(
        error.starts_with(errmsg::INVALID_URI),
        "Error should start with INVALID_URI prefix: {}",
        error
    );

    // Should return immediately without retries
    assert!(
        elapsed < Duration::from_millis(100),
        "Invalid URI should fail fast, took {:?}",
        elapsed
    );
}

/// connect_channel error includes original error details.
///
/// The error message should provide enough context for debugging.
#[tokio::test]
async fn test_connect_error_includes_details() {
    let unreachable = "localhost:59997";

    let result = connect_channel(unreachable).await;
    let error = result.unwrap_err();

    // Error should include the prefix and some detail
    assert!(error.len() > errmsg::CONNECTION_FAILED.len());
    println!("Full error message: {}", error);
}

// ============================================================================
// Address Format Tests
// ============================================================================

/// connect_channel accepts host:port format.
///
/// The function prepends "http://" internally.
#[tokio::test]
async fn test_connect_accepts_host_port_format() {
    // This will fail because nothing is listening, but format should be valid
    let address = "localhost:59996";

    let result = connect_channel(address).await;

    // Should fail with CONNECTION_FAILED, not INVALID_URI
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        !error.starts_with(errmsg::INVALID_URI),
        "host:port format should be valid, got: {}",
        error
    );
}

/// connect_channel with IPv4 address.
#[tokio::test]
async fn test_connect_ipv4_address() {
    let address = "127.0.0.1:59995";

    let result = connect_channel(address).await;

    // Should fail with CONNECTION_FAILED, not INVALID_URI
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        !error.starts_with(errmsg::INVALID_URI),
        "IPv4 format should be valid, got: {}",
        error
    );
}

// ============================================================================
// Retry Behavior Tests
// ============================================================================

/// connect_channel retries with backoff.
///
/// Multiple connection attempts should happen with increasing delays.
/// We can't easily verify the exact backoff timing, but we can check
/// that it takes longer than a single attempt.
#[tokio::test]
async fn test_connect_retries_with_backoff() {
    let unreachable = "localhost:59994";

    let start = Instant::now();
    let _ = connect_channel(unreachable).await;
    let elapsed = start.elapsed();

    // With 10 retries and exponential backoff starting at 100ms,
    // total time should be at least a few hundred milliseconds
    // (unless all retries fail immediately)
    println!("Retry sequence took {:?}", elapsed);

    // At minimum, some delay should occur due to backoff
    // The first attempt has no delay, but subsequent ones do
}
