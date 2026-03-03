//! Tests for resource limits configuration.
//!
//! Resource limits prevent unbounded memory usage and ensure messages
//! fit within message bus constraints. Defaults are conservative (256 KB)
//! to work with SQS/SNS. Higher limits available for other backends.
//!
//! Why this matters: Without payload limits, a single large message can
//! exhaust memory or exceed bus limits, causing silent failures. Each
//! bus backend has different limits; defaults target the most restrictive.
//!
//! Key limits:
//! - Payload size: Per command/event page (bus-dependent)
//! - Pages per book: Bounds batch sizes
//! - Query result limit: Prevents unbounded result sets
//! - Channel capacity: In-process bus queue depth

use super::*;

// ============================================================================
// Default Limits Tests
// ============================================================================

/// Default limits target SQS/SNS (most restrictive bus).
///
/// 256 KB is the hard AWS limit for SQS messages. Using this as
/// default ensures configurations work everywhere.
#[test]
fn test_default_limits() {
    let limits = ResourceLimits::default();
    assert_eq!(limits.max_payload_bytes, 256 * 1024); // 256 KB
    assert_eq!(limits.max_pages_per_book, 100);
    assert_eq!(limits.query_result_limit, 1000);
    assert_eq!(limits.channel_capacity, 1024);
}

// ============================================================================
// Per-Backend Preset Tests
// ============================================================================

/// IPC preset allows 10 MB payloads (local communication).
#[test]
fn test_ipc_limits() {
    let limits = ResourceLimits::for_ipc();
    assert_eq!(limits.max_payload_bytes, 10 * 1024 * 1024); // 10 MB
    assert_eq!(limits.max_pages_per_book, 100); // Others unchanged
}

/// AWS preset matches SQS/SNS limit (256 KB).
#[test]
fn test_aws_limits() {
    let limits = ResourceLimits::for_aws();
    assert_eq!(limits.max_payload_bytes, 256 * 1024); // 256 KB
}

/// Pub/Sub preset matches Google Cloud limit (10 MB).
#[test]
fn test_pubsub_limits() {
    let limits = ResourceLimits::for_pubsub();
    assert_eq!(limits.max_payload_bytes, 10 * 1024 * 1024); // 10 MB
}

/// Kafka preset matches default broker config (1 MB).
#[test]
fn test_kafka_limits() {
    let limits = ResourceLimits::for_kafka();
    assert_eq!(limits.max_payload_bytes, 1024 * 1024); // 1 MB
}
