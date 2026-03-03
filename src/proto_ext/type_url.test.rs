//! Tests for type URL utilities.
//!
//! Type URLs identify protobuf message types in Any fields.
//! Format: "type.angzarr.io/{package}.{MessageType}"
//!
//! Key behaviors verified:
//! - for_type() builds correct URLs
//! - strip_prefix() extracts message type from URL

use super::*;

/// for_type() prepends the angzarr type URL prefix.
#[test]
fn test_for_type() {
    assert_eq!(
        for_type("examples.OrderCreated"),
        "type.angzarr.io/examples.OrderCreated"
    );
    assert_eq!(for_type("angzarr.Notification"), NOTIFICATION);
}

/// strip_prefix() removes the angzarr prefix, passes through unknown URLs.
#[test]
fn test_strip_prefix() {
    assert_eq!(
        strip_prefix("type.angzarr.io/examples.OrderCreated"),
        "examples.OrderCreated"
    );
    assert_eq!(strip_prefix(NOTIFICATION), "angzarr.Notification");
    // Unknown prefix passes through
    assert_eq!(strip_prefix("unknown/Type"), "unknown/Type");
}
