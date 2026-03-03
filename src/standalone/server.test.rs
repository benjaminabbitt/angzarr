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
