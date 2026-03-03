//! Tests for protobuf reflection utilities.
//!
//! Proto reflection enables runtime inspection of Any-packed messages:
//! - Type URL parsing extracts message type from "type.googleapis.com/pkg.Type"
//! - Field diffing identifies changed fields between message versions
//! - Disjoint field detection enables commutative merge optimization
//!
//! Why this matters:
//! - State diff: Detect conflicting concurrent updates (optimistic locking)
//! - Logging: Human-readable event/state representation
//! - Debug tooling: Inspect Any-packed messages without static type knowledge
//!
//! Key behaviors verified:
//! - Type URL parsing handles various prefix formats
//! - Field disjointness correctly identifies non-overlapping changes
//! - Map fields use keyed paths like "field[key]" for granular diff
//!
//! Note: Full diff_fields tests require integration tests with real
//! descriptor sets. Unit tests cover parsing and set operations.

use super::*;

// ============================================================================
// Type URL Parsing Tests
// ============================================================================

/// Extract type name from googleapis.com format.
///
/// Standard protobuf Any type URL format.
#[test]
fn test_extract_type_name_googleapis() {
    let type_url = "type.googleapis.com/examples.PlayerState";
    let result = extract_type_name(type_url).unwrap();
    assert_eq!(result, "examples.PlayerState");
}

/// Extract type name from angzarr.io format.
///
/// Custom type URLs used for angzarr-specific messages.
#[test]
fn test_extract_type_name_angzarr() {
    use crate::proto_ext::type_url;
    let result = extract_type_name(type_url::SAGA_COMPENSATION_FAILED).unwrap();
    assert_eq!(result, "angzarr.SagaCompensationFailed");
}

/// Edge case: bare type name without prefix still works.
///
/// Handles malformed or simplified type URLs gracefully.
#[test]
fn test_extract_type_name_just_name() {
    // Edge case: no prefix
    let type_url = "examples.PlayerState";
    let result = extract_type_name(type_url).unwrap();
    assert_eq!(result, "examples.PlayerState");
}

// ============================================================================
// Field Disjointness Tests
// ============================================================================
//
// Disjoint fields enable commutative merge: if two concurrent updates
// touch different fields, they can be applied in any order.

/// Empty field sets are trivially disjoint.
#[test]
fn test_fields_are_disjoint_empty() {
    let a: HashSet<String> = HashSet::new();
    let b: HashSet<String> = HashSet::new();
    assert!(fields_are_disjoint(&a, &b));
}

/// Different scalar fields are disjoint (can merge).
///
/// Example: One update changes "bankroll", another changes "name".
/// No conflict; both can be applied.
#[test]
fn test_fields_are_disjoint_different_fields() {
    let a: HashSet<String> = ["bankroll".to_string()].into_iter().collect();
    let b: HashSet<String> = ["name".to_string()].into_iter().collect();
    assert!(fields_are_disjoint(&a, &b));
}

/// Same field in both sets → conflict (cannot merge).
///
/// Example: Both updates change "bankroll". Last-write-wins or reject.
#[test]
fn test_fields_are_disjoint_same_field() {
    let a: HashSet<String> = ["bankroll".to_string()].into_iter().collect();
    let b: HashSet<String> = ["bankroll".to_string()].into_iter().collect();
    assert!(!fields_are_disjoint(&a, &b));
}

/// Different keys in same map → disjoint (key-level granularity).
///
/// Map fields track changes per-key: seats[1] and seats[2] don't conflict.
#[test]
fn test_fields_are_disjoint_keyed_different_keys() {
    // Different keys in same map → disjoint
    let a: HashSet<String> = ["seats[1]".to_string()].into_iter().collect();
    let b: HashSet<String> = ["seats[2]".to_string()].into_iter().collect();
    assert!(fields_are_disjoint(&a, &b));
}

/// Same key in same map → conflict.
///
/// Both updates modify seats[1]; conflict detected.
#[test]
fn test_fields_are_disjoint_keyed_same_key() {
    // Same key → overlap
    let a: HashSet<String> = ["seats[1]".to_string()].into_iter().collect();
    let b: HashSet<String> = ["seats[1]".to_string()].into_iter().collect();
    assert!(!fields_are_disjoint(&a, &b));
}

/// Mixed scalar and keyed fields: all different → disjoint.
#[test]
fn test_fields_are_disjoint_mixed() {
    let a: HashSet<String> = ["bankroll".to_string(), "seats[1]".to_string()]
        .into_iter()
        .collect();
    let b: HashSet<String> = ["name".to_string(), "seats[2]".to_string()]
        .into_iter()
        .collect();
    assert!(fields_are_disjoint(&a, &b));
}

/// Mixed scalar and keyed fields: one overlap → conflict.
#[test]
fn test_fields_are_disjoint_mixed_overlap() {
    let a: HashSet<String> = ["bankroll".to_string(), "seats[1]".to_string()]
        .into_iter()
        .collect();
    let b: HashSet<String> = ["seats[1]".to_string(), "name".to_string()]
        .into_iter()
        .collect();
    assert!(!fields_are_disjoint(&a, &b));
}

// ============================================================================
// Map Key Formatting Tests
// ============================================================================

/// String map keys format as-is.
#[test]
fn test_format_map_key_string() {
    use prost_reflect::MapKey;
    let key = MapKey::String("table_a".to_string());
    assert_eq!(format_map_key(&key), "table_a");
}

/// Integer map keys format as decimal strings.
#[test]
fn test_format_map_key_i32() {
    use prost_reflect::MapKey;
    let key = MapKey::I32(42);
    assert_eq!(format_map_key(&key), "42");
}

/// Unsigned 64-bit map keys format as decimal strings.
#[test]
fn test_format_map_key_u64() {
    use prost_reflect::MapKey;
    let key = MapKey::U64(123456);
    assert_eq!(format_map_key(&key), "123456");
}

// ============================================================================
// Pool Initialization Tests
// ============================================================================
//
// Note: These tests verify error types, not actual initialization.
// Global static makes initialization tests unreliable in parallel test runs.
// Full reflection tests belong in integration tests with descriptor sets.

/// NotInitialized error has correct message.
#[test]
fn test_pool_not_initialized_error() {
    // In a fresh test process where pool isn't initialized,
    // this would return NotInitialized. However, other tests
    // may have initialized it. We test the error type exists.
    let err = ReflectError::NotInitialized;
    assert_eq!(err.to_string(), errmsg::NOT_INITIALIZED);
}

/// AlreadyInitialized error has correct message.
#[test]
fn test_already_initialized_error() {
    let err = ReflectError::AlreadyInitialized;
    assert_eq!(err.to_string(), errmsg::ALREADY_INITIALIZED);
}

/// UnknownType error includes the type name.
///
/// Diagnostic: Helps identify which proto type is missing from descriptors.
#[test]
fn test_unknown_type_error() {
    let err = ReflectError::UnknownType("foo.Bar".to_string());
    assert_eq!(err.to_string(), format!("{}foo.Bar", errmsg::UNKNOWN_TYPE));
}

// ============================================================================
// Integration Test Scaffolding
// ============================================================================
//
// Full diff_fields tests require:
// 1. Generated descriptor.bin from protoc
// 2. Test proto messages with known field structures
//
// These will be added as integration tests in tests/standalone_integration/state_diff.rs
