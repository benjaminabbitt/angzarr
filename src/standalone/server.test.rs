//! Tests for standalone gRPC server utilities.
//!
//! The standalone server provides per-domain implementations of AggregateCoordinator
//! and EventQuery services. These wrap the CommandRouter and domain stores to provide
//! a gRPC interface identical to distributed mode.
//!
//! Why this matters: Commands must be routed to the correct domain handler. If domain
//! validation fails, commands could execute against wrong aggregates. If temporal
//! queries parse incorrectly, speculative execution returns wrong state.
//!
//! Key behaviors verified:
//! - Domain validation catches mismatched command/service domains
//! - Error messages include both domains for debugging
//! - Temporal query parsing handles all variants (sequence, timestamp, none)

use super::*;

// ============================================================================
// validate_domain_match Tests
// ============================================================================

/// Matching domains pass validation.
#[test]
fn test_validate_domain_match_same_domain_succeeds() {
    let result = validate_domain_match("orders", "orders", "Command");
    assert!(result.is_ok());
}

/// Mismatched domains return InvalidArgument.
///
/// This prevents commands from being routed to the wrong handler.
/// The error message must include both domains for debugging.
#[test]
fn test_validate_domain_match_different_domain_fails() {
    let result = validate_domain_match("inventory", "orders", "Command");
    assert!(result.is_err());
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
    assert!(status.message().contains("inventory"));
    assert!(status.message().contains("orders"));
}

/// Empty domain is a mismatch - commands must have a domain.
#[test]
fn test_validate_domain_match_empty_domain_fails() {
    let result = validate_domain_match("", "orders", "Query");
    assert!(result.is_err());
}

/// Context string appears in error message for debugging.
///
/// Distinguishes Command vs Query vs Event validation failures.
#[test]
fn test_validate_domain_match_context_in_message() {
    let result = validate_domain_match("a", "b", "Event");
    let status = result.unwrap_err();
    assert!(status.message().contains("Event"));
}

// ============================================================================
// parse_temporal_query Tests
// ============================================================================

/// None temporal query returns (None, None) - use current state.
#[test]
fn test_parse_temporal_query_none_returns_none_none() {
    let (seq, ts) = parse_temporal_query(None);
    assert!(seq.is_none());
    assert!(ts.is_none());
}

/// AsOfSequence extracts the sequence number.
///
/// Used to replay aggregate state up to a specific event.
#[test]
fn test_parse_temporal_query_as_of_sequence() {
    use crate::proto::{temporal_query::PointInTime, TemporalQuery};

    let temporal = TemporalQuery {
        point_in_time: Some(PointInTime::AsOfSequence(42)),
    };
    let (seq, ts) = parse_temporal_query(Some(&temporal));
    assert_eq!(seq, Some(42));
    assert!(ts.is_none());
}

/// AsOfTime extracts timestamp as "seconds.nanos" string.
///
/// Used to replay aggregate state as it existed at a point in time.
#[test]
fn test_parse_temporal_query_as_of_time() {
    use crate::proto::{temporal_query::PointInTime, TemporalQuery};

    let temporal = TemporalQuery {
        point_in_time: Some(PointInTime::AsOfTime(prost_types::Timestamp {
            seconds: 1704067200,
            nanos: 123456789,
        })),
    };
    let (seq, ts) = parse_temporal_query(Some(&temporal));
    assert!(seq.is_none());
    assert_eq!(ts, Some("1704067200.123456789".to_string()));
}

/// TemporalQuery with None point_in_time returns (None, None).
#[test]
fn test_parse_temporal_query_empty_point_in_time() {
    use crate::proto::TemporalQuery;

    let temporal = TemporalQuery {
        point_in_time: None,
    };
    let (seq, ts) = parse_temporal_query(Some(&temporal));
    assert!(seq.is_none());
    assert!(ts.is_none());
}

// ============================================================================
// parse_query_cover Tests
// ============================================================================

/// Query without cover returns InvalidArgument.
///
/// Every query must specify which aggregate to query.
#[test]
fn test_parse_query_cover_missing_cover_returns_error() {
    use crate::proto::Query;

    let query = Query {
        cover: None,
        selection: None,
    };
    let result = parse_query_cover(&query);
    assert!(result.is_err());
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

/// Query with cover but missing root returns InvalidArgument.
///
/// Root ID identifies the aggregate instance.
#[test]
fn test_parse_query_cover_missing_root_returns_error() {
    use crate::proto::{Cover, Query};

    let query = Query {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: None,
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };
    let result = parse_query_cover(&query);
    assert!(result.is_err());
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

/// Query with invalid UUID bytes returns InvalidArgument.
///
/// UUID must be exactly 16 bytes.
#[test]
fn test_parse_query_cover_invalid_uuid_returns_error() {
    use crate::proto::{Cover, Query, Uuid as ProtoUuid};

    let query = Query {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: vec![1, 2, 3], // Too short
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };
    let result = parse_query_cover(&query);
    assert!(result.is_err());
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
    assert!(status.message().contains("UUID"));
}

/// Valid query returns edition and root UUID.
#[test]
fn test_parse_query_cover_valid_query_returns_edition_and_root() {
    use crate::proto::{Cover, Edition, Query, Uuid as ProtoUuid};

    let root = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let query = Query {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: Some(Edition {
                name: "test-edition".to_string(),
                divergences: vec![],
            }),
        }),
        selection: None,
    };
    let result = parse_query_cover(&query);
    assert!(result.is_ok());
    let (edition, parsed_root) = result.unwrap();
    assert_eq!(edition, Some("test-edition"));
    assert_eq!(parsed_root, root);
}

/// Query without edition returns None for edition.
#[test]
fn test_parse_query_cover_no_edition_returns_none() {
    use crate::proto::{Cover, Query, Uuid as ProtoUuid};

    let root = uuid::Uuid::new_v4();
    let query = Query {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None,
    };
    let result = parse_query_cover(&query);
    assert!(result.is_ok());
    let (edition, _) = result.unwrap();
    assert!(edition.is_none());
}
