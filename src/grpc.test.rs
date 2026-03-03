//! Tests for gRPC utilities.
//!
//! The gRPC module provides connection utilities with retry and backoff.
//! Error messages are constants for consistency across the codebase.
//!
//! Why this matters: Consistent error messages enable reliable error
//! handling and testing. Connection retry logic must handle transient
//! failures gracefully.

use super::*;

// ============================================================================
// Error Message Constant Tests
// ============================================================================

/// Error message constants have expected values.
///
/// These constants are used for error matching in tests and error handling.
#[test]
fn test_error_message_constants() {
    assert_eq!(errmsg::CONNECTION_FAILED, "Connection failed: ");
    assert_eq!(errmsg::INVALID_URI, "Invalid URI: ");
    assert_eq!(
        errmsg::MAX_RETRIES_EXCEEDED,
        "Connection failed after max retries"
    );
}

/// CONNECTION_FAILED is a prefix for connection error messages.
#[test]
fn test_connection_failed_is_prefix() {
    let error = format!("{}timeout", errmsg::CONNECTION_FAILED);
    assert!(error.starts_with(errmsg::CONNECTION_FAILED));
}

/// INVALID_URI is a prefix for URI parsing error messages.
#[test]
fn test_invalid_uri_is_prefix() {
    let error = format!("{}bad syntax", errmsg::INVALID_URI);
    assert!(error.starts_with(errmsg::INVALID_URI));
}
