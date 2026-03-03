//! Tests for DLQ publisher factory.
//!
//! The factory uses self-registration (inventory crate) to discover available
//! DLQ backends at compile time. This enables modular backend support without
//! explicit registration.
//!
//! Why this matters: New DLQ backends can be added without modifying the
//! factory code. Feature flags control which backends are compiled in.
//!
//! Key behaviors verified:
//! - Empty config returns noop publisher
//! - Unknown backend types are rejected
//! - Single target returns publisher directly (no chaining overhead)

use super::*;

/// Empty config returns noop publisher.
///
/// No DLQ configured means dead letters are silently discarded.
/// is_configured() returns false to signal this to callers.
#[tokio::test]
async fn test_init_dlq_publisher_empty_config() {
    let config = DlqConfig::default();
    let publisher = init_dlq_publisher(&config).await.unwrap();
    assert!(!publisher.is_configured());
}
