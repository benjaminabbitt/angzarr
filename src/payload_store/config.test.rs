//! Tests for payload store configuration.
//!
//! Payload offloading is the claim check pattern: when payloads exceed
//! message bus size limits, store them externally and pass references.
//!
//! Why this matters: Configuration controls when and where payloads are
//! stored. Misconfiguration could either waste storage (offloading small
//! payloads) or break message transmission (not offloading large ones).
//!
//! Key behaviors verified:
//! - Default config is disabled (opt-in feature)
//! - Threshold = 0 means use bus default
//! - Duration helpers convert hours/seconds correctly
//! - YAML deserialization works with full and minimal configs

use super::*;

// ============================================================================
// PayloadOffloadConfig Tests
// ============================================================================

/// Default config has offloading disabled.
///
/// Payload offloading requires external storage setup. Disabled by default
/// so the system works out-of-box without additional infrastructure.
#[test]
fn test_payload_offload_config_default() {
    let config = PayloadOffloadConfig::default();

    assert!(!config.enabled);
    assert_eq!(config.store_type, PayloadStoreType::Filesystem);
    assert_eq!(config.threshold_bytes, 0);
    assert_eq!(config.retention_hours, 24);
    assert_eq!(config.cleanup_interval_secs, 3600);
}

/// Zero threshold means use the bus's max_message_size.
///
/// Different buses have different limits (SQS: 256KB, Kafka: 1MB, etc.).
/// Setting threshold=0 defers to the bus, avoiding misconfiguration.
#[test]
fn test_payload_offload_config_threshold() {
    let mut config = PayloadOffloadConfig::default();

    // Zero threshold means use bus default
    assert_eq!(config.threshold(), None);

    // Explicit threshold
    config.threshold_bytes = 1024;
    assert_eq!(config.threshold(), Some(1024));
}

/// Retention converts hours to Duration.
#[test]
fn test_payload_offload_config_retention() {
    let config = PayloadOffloadConfig {
        retention_hours: 48,
        ..Default::default()
    };

    assert_eq!(config.retention(), Duration::from_secs(48 * 3600));
}

/// Cleanup interval converts seconds to Duration.
#[test]
fn test_payload_offload_config_cleanup_interval() {
    let config = PayloadOffloadConfig {
        cleanup_interval_secs: 300,
        ..Default::default()
    };

    assert_eq!(config.cleanup_interval(), Duration::from_secs(300));
}

// ============================================================================
// FilesystemStoreConfig Tests
// ============================================================================

/// Default filesystem path is /var/angzarr/payloads.
#[test]
fn test_filesystem_store_config_default() {
    let config = FilesystemStoreConfig::default();

    assert_eq!(config.base_path, PathBuf::from("/var/angzarr/payloads"));
}

// ============================================================================
// YAML Deserialization Tests
// ============================================================================

/// Full YAML config deserializes all fields correctly.
#[test]
fn test_payload_offload_config_deserialize_yaml() {
    let yaml = r#"
        enabled: true
        type: filesystem
        threshold_bytes: 262144
        retention_hours: 72
        cleanup_interval_secs: 1800
        filesystem:
          base_path: /tmp/payloads
    "#;

    let config: PayloadOffloadConfig = serde_yaml::from_str(yaml).unwrap();

    assert!(config.enabled);
    assert_eq!(config.store_type, PayloadStoreType::Filesystem);
    assert_eq!(config.threshold_bytes, 262144);
    assert_eq!(config.retention_hours, 72);
    assert_eq!(config.cleanup_interval_secs, 1800);
    assert_eq!(config.filesystem.base_path, PathBuf::from("/tmp/payloads"));
}

/// Minimal YAML config uses defaults for unspecified fields.
#[test]
fn test_payload_offload_config_deserialize_minimal_yaml() {
    let yaml = r#"
        enabled: true
    "#;

    let config: PayloadOffloadConfig = serde_yaml::from_str(yaml).unwrap();

    assert!(config.enabled);
    // All other fields should have defaults
    assert_eq!(config.store_type, PayloadStoreType::Filesystem);
    assert_eq!(config.retention_hours, 24);
}

/// Store type enum deserializes from lowercase strings.
#[test]
fn test_payload_store_type_deserialize() {
    let yaml = "filesystem";
    let store_type: PayloadStoreType = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(store_type, PayloadStoreType::Filesystem);
}
