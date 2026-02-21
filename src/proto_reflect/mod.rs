//! Protobuf reflection utilities.
//!
//! Provides runtime introspection of Any-packed protobuf messages
//! using a shared DescriptorPool. Used for:
//! - State diff (commutative merge detection)
//! - Logging projectors (human-readable event/state logging)
//! - Debug tooling (inspect Any-packed messages)

use prost_reflect::ReflectMessage;
use prost_types::Any;
use std::collections::HashSet;
use std::sync::OnceLock;

/// Global descriptor pool, initialized at startup.
static DESCRIPTOR_POOL: OnceLock<prost_reflect::DescriptorPool> = OnceLock::new();

/// Errors that can occur during proto reflection.
#[derive(Debug, thiserror::Error)]
pub enum ReflectError {
    #[error("Descriptor pool not initialized")]
    NotInitialized,

    #[error("Descriptor pool already initialized")]
    AlreadyInitialized,

    #[error("Invalid type URL: {0}")]
    InvalidTypeUrl(String),

    #[error("Unknown message type: {0}")]
    UnknownType(String),

    #[error("Decode error: {0}")]
    Decode(#[from] prost::DecodeError),

    #[error("Pool decode error: {0}")]
    PoolDecode(String),
}

/// Initialize the descriptor pool from file descriptor set bytes.
///
/// Call once at coordinator/runtime startup.
///
/// # Errors
///
/// Returns error if pool is already initialized or if bytes are invalid.
pub fn init_pool(fds_bytes: &[u8]) -> Result<(), ReflectError> {
    let pool = prost_reflect::DescriptorPool::decode(fds_bytes)
        .map_err(|e| ReflectError::PoolDecode(e.to_string()))?;
    DESCRIPTOR_POOL
        .set(pool)
        .map_err(|_| ReflectError::AlreadyInitialized)
}

/// Get the global descriptor pool.
///
/// # Errors
///
/// Returns error if pool is not initialized.
pub fn pool() -> Result<&'static prost_reflect::DescriptorPool, ReflectError> {
    DESCRIPTOR_POOL.get().ok_or(ReflectError::NotInitialized)
}

/// Check if the descriptor pool is initialized.
pub fn is_initialized() -> bool {
    DESCRIPTOR_POOL.get().is_some()
}

/// Embedded descriptor set from build time.
///
/// Contains all proto message definitions compiled into the binary.
/// Use `init_from_embedded()` to initialize the pool with this data.
pub const EMBEDDED_DESCRIPTOR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/descriptor.bin"));

/// Initialize the descriptor pool from the embedded descriptor set.
///
/// This is the recommended way to initialize the pool at startup.
/// Call this once during application initialization.
///
/// # Example
///
/// ```ignore
/// if let Err(e) = angzarr::proto_reflect::init_from_embedded() {
///     tracing::warn!(error = %e, "Failed to initialize proto reflection");
/// }
/// ```
///
/// # Errors
///
/// Returns error if pool is already initialized or if descriptor is invalid.
pub fn init_from_embedded() -> Result<(), ReflectError> {
    init_pool(EMBEDDED_DESCRIPTOR)
}

/// Extract type name from Any.type_url.
///
/// "type.googleapis.com/examples.PlayerState" -> "examples.PlayerState"
fn extract_type_name(type_url: &str) -> Result<&str, ReflectError> {
    type_url
        .rsplit('/')
        .next()
        .ok_or_else(|| ReflectError::InvalidTypeUrl(type_url.to_string()))
}

/// Decode an Any into a DynamicMessage for reflection.
///
/// # Errors
///
/// Returns error if pool is not initialized, type is unknown, or decode fails.
pub fn decode_any(any: &Any) -> Result<prost_reflect::DynamicMessage, ReflectError> {
    let pool = pool()?;
    let type_name = extract_type_name(&any.type_url)?;
    let descriptor = pool
        .get_message_by_name(type_name)
        .ok_or_else(|| ReflectError::UnknownType(type_name.to_string()))?;
    Ok(prost_reflect::DynamicMessage::decode(
        descriptor,
        &any.value[..],
    )?)
}

/// Compare two Any-packed messages, return changed field paths.
///
/// Handles scalar fields and map fields with key-based paths.
/// Returns field paths like "bankroll", "seats[1]", "table_reservations[table_a]".
///
/// # Errors
///
/// Returns error if either message cannot be decoded.
pub fn diff_fields(before: &Any, after: &Any) -> Result<HashSet<String>, ReflectError> {
    let before_msg = decode_any(before)?;
    let after_msg = decode_any(after)?;
    Ok(diff_dynamic_messages(&before_msg, &after_msg))
}

/// Compare two DynamicMessages and return changed field paths.
fn diff_dynamic_messages(
    before: &prost_reflect::DynamicMessage,
    after: &prost_reflect::DynamicMessage,
) -> HashSet<String> {
    let mut changed = HashSet::new();

    for field in before.descriptor().fields() {
        let before_val = before.get_field(&field);
        let after_val = after.get_field(&field);

        if field.is_map() {
            // Map field: check each key individually
            diff_map_field(&field, &before_val, &after_val, &mut changed);
        } else if before_val != after_val {
            changed.insert(field.name().to_string());
        }
    }

    // Note: We assume both messages are the same type.
    // If types differ, the descriptor iteration above handles all fields.

    changed
}

/// Diff a map field, adding "field[key]" paths for changed entries.
fn diff_map_field(
    field: &prost_reflect::FieldDescriptor,
    before: &prost_reflect::Value,
    after: &prost_reflect::Value,
    changed: &mut HashSet<String>,
) {
    use prost_reflect::Value;

    let before_map = match before {
        Value::Map(m) => m.clone(),
        _ => return,
    };
    let after_map = match after {
        Value::Map(m) => m.clone(),
        _ => return,
    };

    // Keys in after but not before, or with different values
    for (key, after_val) in after_map.iter() {
        match before_map.get(key) {
            Some(before_val) if before_val == after_val => {}
            _ => {
                // Changed or added
                let key_str = format_map_key(key);
                changed.insert(format!("{}[{}]", field.name(), key_str));
            }
        }
    }

    // Keys removed (in before but not after)
    for key in before_map.keys() {
        if !after_map.contains_key(key) {
            let key_str = format_map_key(key);
            changed.insert(format!("{}[{}]", field.name(), key_str));
        }
    }
}

/// Format a map key for display in field paths.
fn format_map_key(key: &prost_reflect::MapKey) -> String {
    use prost_reflect::MapKey;
    match key {
        MapKey::Bool(b) => b.to_string(),
        MapKey::I32(n) => n.to_string(),
        MapKey::I64(n) => n.to_string(),
        MapKey::U32(n) => n.to_string(),
        MapKey::U64(n) => n.to_string(),
        MapKey::String(s) => s.clone(),
    }
}

/// Check if two sets of changed fields are disjoint (no overlap).
///
/// Used for commutative merge detection.
pub fn fields_are_disjoint(a: &HashSet<String>, b: &HashSet<String>) -> bool {
    a.is_disjoint(b)
}

/// Format a DynamicMessage as human-readable string (for logging).
pub fn format_message(msg: &prost_reflect::DynamicMessage) -> String {
    format!("{:?}", msg)
}

/// Format an Any as human-readable string.
///
/// # Errors
///
/// Returns error if message cannot be decoded.
pub fn format_any(any: &Any) -> Result<String, ReflectError> {
    let msg = decode_any(any)?;
    Ok(format_message(&msg))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Type URL Parsing Tests
    // ============================================================================

    #[test]
    fn test_extract_type_name_googleapis() {
        let type_url = "type.googleapis.com/examples.PlayerState";
        let result = extract_type_name(type_url).unwrap();
        assert_eq!(result, "examples.PlayerState");
    }

    #[test]
    fn test_extract_type_name_angzarr() {
        let type_url = "type.angzarr/angzarr.SagaCompensationFailed";
        let result = extract_type_name(type_url).unwrap();
        assert_eq!(result, "angzarr.SagaCompensationFailed");
    }

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

    #[test]
    fn test_fields_are_disjoint_empty() {
        let a: HashSet<String> = HashSet::new();
        let b: HashSet<String> = HashSet::new();
        assert!(fields_are_disjoint(&a, &b));
    }

    #[test]
    fn test_fields_are_disjoint_different_fields() {
        let a: HashSet<String> = ["bankroll".to_string()].into_iter().collect();
        let b: HashSet<String> = ["name".to_string()].into_iter().collect();
        assert!(fields_are_disjoint(&a, &b));
    }

    #[test]
    fn test_fields_are_disjoint_same_field() {
        let a: HashSet<String> = ["bankroll".to_string()].into_iter().collect();
        let b: HashSet<String> = ["bankroll".to_string()].into_iter().collect();
        assert!(!fields_are_disjoint(&a, &b));
    }

    #[test]
    fn test_fields_are_disjoint_keyed_different_keys() {
        // Different keys in same map → disjoint
        let a: HashSet<String> = ["seats[1]".to_string()].into_iter().collect();
        let b: HashSet<String> = ["seats[2]".to_string()].into_iter().collect();
        assert!(fields_are_disjoint(&a, &b));
    }

    #[test]
    fn test_fields_are_disjoint_keyed_same_key() {
        // Same key → overlap
        let a: HashSet<String> = ["seats[1]".to_string()].into_iter().collect();
        let b: HashSet<String> = ["seats[1]".to_string()].into_iter().collect();
        assert!(!fields_are_disjoint(&a, &b));
    }

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

    #[test]
    fn test_format_map_key_string() {
        use prost_reflect::MapKey;
        let key = MapKey::String("table_a".to_string());
        assert_eq!(format_map_key(&key), "table_a");
    }

    #[test]
    fn test_format_map_key_i32() {
        use prost_reflect::MapKey;
        let key = MapKey::I32(42);
        assert_eq!(format_map_key(&key), "42");
    }

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
    // Note: These tests are tricky because of the global static.
    // In production, pool is initialized once at startup.
    // Tests that need actual reflection should use integration tests
    // with proper descriptor set files.

    #[test]
    fn test_pool_not_initialized_error() {
        // In a fresh test process where pool isn't initialized,
        // this would return NotInitialized. However, other tests
        // may have initialized it. We test the error type exists.
        let err = ReflectError::NotInitialized;
        assert!(err.to_string().contains("not initialized"));
    }

    #[test]
    fn test_already_initialized_error() {
        let err = ReflectError::AlreadyInitialized;
        assert!(err.to_string().contains("already initialized"));
    }

    #[test]
    fn test_unknown_type_error() {
        let err = ReflectError::UnknownType("foo.Bar".to_string());
        assert!(err.to_string().contains("foo.Bar"));
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
}
