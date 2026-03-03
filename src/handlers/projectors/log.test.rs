//! Tests for the logging projector's hex dump functionality.
//!
//! The logging projector pretty-prints events for debugging. When protobuf
//! descriptors aren't available, events are displayed as hex dumps.
//! These tests verify hex formatting handles edge cases correctly.

use super::*;

// ============================================================================
// hex_dump Tests
// ============================================================================

/// Verify hex_dump shows full content for small payloads.
///
/// Small events should display completely without truncation,
/// making debugging easier for typical protobuf messages.
#[test]
fn test_hex_dump_small_payload() {
    let service = LogService::with_output(Box::new(super::super::output::StdoutOutput));
    let bytes = vec![0xDE, 0xAD, 0xBE, 0xEF];

    let result = service.hex_dump(&bytes);

    assert!(result.contains("4 bytes"));
    assert!(result.contains("deadbeef"));
    assert!(
        !result.contains("..."),
        "Small payload should not be truncated"
    );
}

/// Verify hex_dump truncates large payloads at 64 bytes.
///
/// Large binary blobs would clutter logs. The 64-byte preview gives
/// enough context for debugging while keeping output manageable.
#[test]
fn test_hex_dump_large_payload_truncates() {
    let service = LogService::with_output(Box::new(super::super::output::StdoutOutput));
    let bytes = vec![0xAB; 100]; // 100 bytes

    let result = service.hex_dump(&bytes);

    assert!(result.contains("100 bytes"));
    assert!(result.contains("..."), "Large payload should be truncated");
    // Should show exactly 64 bytes = 128 hex chars
    assert!(result.contains(&"ab".repeat(64)));
}

/// Verify hex_dump handles empty payloads gracefully.
///
/// Empty events can occur in edge cases. Should not panic or
/// produce malformed output.
#[test]
fn test_hex_dump_empty_payload() {
    let service = LogService::with_output(Box::new(super::super::output::StdoutOutput));
    let bytes: Vec<u8> = vec![];

    let result = service.hex_dump(&bytes);

    assert!(result.contains("0 bytes"));
}

/// Verify hex_dump boundary case: exactly 64 bytes shows no truncation.
#[test]
fn test_hex_dump_exactly_64_bytes() {
    let service = LogService::with_output(Box::new(super::super::output::StdoutOutput));
    let bytes = vec![0xCD; 64];

    let result = service.hex_dump(&bytes);

    assert!(result.contains("64 bytes"));
    assert!(
        !result.contains("..."),
        "Exactly 64 bytes should not truncate"
    );
}
