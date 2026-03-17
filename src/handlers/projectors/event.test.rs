//! Tests for the generic Event projector.
//!
//! The Event projector writes all events as JSON to a database table,
//! enabling ad-hoc querying and debugging. These tests verify:
//! - Base64 encoding for binary fallback when descriptors unavailable
//! - Event type extraction from fully-qualified type URLs
//! - SQL query building with proper escaping (SQL injection prevention)
//!
//! Database integration tests are in Gherkin interface tests.

use super::*;

// ============================================================================
// Base64 Encoding Tests
// ============================================================================

/// Base64 encoding for binary event fallback.
///
/// When protobuf descriptors aren't available to decode events to JSON,
/// we store the binary as base64. Standard test vectors from RFC 4648.
#[test]
fn test_base64_encode() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

// ============================================================================
// Type URL Parsing Tests
// ============================================================================

/// Event type extraction strips URL prefix.
///
/// Protobuf Any messages use URLs like "type.googleapis.com/orders.OrderCreated".
/// For storage and querying, we want just "orders.OrderCreated".
#[test]
fn test_extract_event_type() {
    assert_eq!(
        EventService::extract_event_type("type.googleapis.com/orders.OrderCreated"),
        "orders.OrderCreated"
    );
    assert_eq!(
        EventService::extract_event_type("OrderCreated"),
        "OrderCreated"
    );
}

// ============================================================================
// SQL Query Building Tests
// ============================================================================

/// INSERT statement includes all event fields.
///
/// Verifies sea-query builds valid SQL with all required columns.
/// Uses ON CONFLICT DO NOTHING for idempotent inserts.
#[test]
fn test_event_record_build_insert() {
    let record = EventRecord {
        domain: "orders",
        root_id: "abc123",
        sequence: 42,
        event_type: "OrderCreated",
        event_json: r#"{"id": 1}"#,
        correlation_id: "corr-456",
        created_at: "2024-01-01T00:00:00Z",
    };

    let stmt = record.build_insert().expect("should build insert");
    let sql = build_query(stmt);

    assert!(sql.contains("INSERT INTO"));
    assert!(sql.contains("events"));
    assert!(sql.contains("orders"));
    assert!(sql.contains("abc123"));
    assert!(sql.contains("42"));
    assert!(sql.contains("OrderCreated"));
}

/// SQL injection in domain name is safely escaped.
///
/// Critical security test: untrusted input in domain/root_id/event_json
/// must not allow SQL injection. Sea-query escapes appropriately per
/// backend (Postgres backslash, SQLite double-quote).
#[test]
fn test_event_record_escapes_special_characters() {
    let record = EventRecord {
        domain: "test'; DROP TABLE events;--",
        root_id: "root",
        sequence: 1,
        event_type: "Event",
        event_json: "{}",
        correlation_id: "corr",
        created_at: "2024-01-01T00:00:00Z",
    };

    let stmt = record.build_insert().expect("should build insert");
    let sql = build_query(stmt);

    // Sea-query escapes special characters appropriately for each backend:
    // - PostgreSQL: uses E'...\'..' syntax (backslash escaping)
    // - SQLite: uses '..''...' syntax (doubled quotes)
    // Either way, the injection payload cannot execute because it's safely escaped.
    #[cfg(feature = "postgres")]
    {
        assert!(
            sql.contains(r"E'test\'; DROP TABLE"),
            "PostgreSQL should use backslash escape: {sql}"
        );
    }
    #[cfg(not(feature = "postgres"))]
    {
        assert!(
            sql.contains("''"),
            "SQLite should double single quotes: {sql}"
        );
        assert!(
            sql.contains("test''; DROP TABLE"),
            "injection payload should be safely escaped: {sql}"
        );
    }
}
