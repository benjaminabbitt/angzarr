//! Tests for DLQ configuration parsing and factory methods.
//!
//! DLQ is configured as a priority list of targets. Each target is tried in
//! order until one succeeds. This enables fallback patterns: try message queue
//! first, fall back to database, then filesystem, then logging.
//!
//! Why this matters: Configuration correctness is critical — misconfigured DLQ
//! silently drops dead letters, making debugging impossible. These tests verify
//! factory methods create correct structures.
//!
//! Key behaviors verified:
//! - Default config is empty (no DLQ)
//! - Factory methods create correct single-target configs
//! - Backend-specific config defaults are sensible

use super::*;

// ============================================================================
// DlqConfig Tests
// ============================================================================

/// Default config has no targets — DLQ disabled.
#[test]
fn test_dlq_config_default_not_configured() {
    let config = DlqConfig::default();
    assert!(!config.is_configured());
    assert!(config.targets.is_empty());
}

/// Channel backend for standalone mode.
#[test]
fn test_dlq_config_channel_configured() {
    let config = DlqConfig::channel();
    assert!(config.is_configured());
    assert_eq!(config.targets.len(), 1);
    assert_eq!(config.targets[0].dlq_type, "channel");
}

/// AMQP backend with custom URL.
#[test]
fn test_dlq_config_amqp_configured() {
    let config = DlqConfig::amqp("amqp://rabbitmq:5672");
    assert!(config.is_configured());
    assert_eq!(config.targets.len(), 1);
    assert_eq!(config.targets[0].dlq_type, "amqp");
    assert_eq!(
        config.targets[0].amqp.as_ref().unwrap().url,
        "amqp://rabbitmq:5672"
    );
}

/// Kafka backend with custom brokers.
#[test]
fn test_dlq_config_kafka_configured() {
    let config = DlqConfig::kafka("kafka:9092");
    assert!(config.is_configured());
    assert_eq!(config.targets.len(), 1);
    assert_eq!(config.targets[0].dlq_type, "kafka");
    assert_eq!(
        config.targets[0].kafka.as_ref().unwrap().bootstrap_servers,
        "kafka:9092"
    );
}

/// Logging backend — last resort, always available.
#[test]
fn test_dlq_config_logging_configured() {
    let config = DlqConfig::logging();
    assert!(config.is_configured());
    assert_eq!(config.targets.len(), 1);
    assert_eq!(config.targets[0].dlq_type, "logging");
}

// ============================================================================
// Backend Config Defaults Tests
// ============================================================================

/// Target config defaults have empty type and no backend configs.
#[test]
fn test_dlq_target_config_default() {
    let target = DlqTargetConfig::default();
    assert!(target.dlq_type.is_empty());
    assert!(target.amqp.is_none());
    assert!(target.kafka.is_none());
    assert!(target.database.is_none());
    assert!(target.filesystem.is_none());
    assert!(target.offload_filesystem.is_none());
    assert!(target.offload_gcs.is_none());
    assert!(target.offload_s3.is_none());
}

/// Database backend defaults to PostgreSQL.
#[test]
fn test_database_dlq_config_default() {
    let config = DatabaseDlqConfig::default();
    assert_eq!(config.storage_type, "postgres");
    assert!(config.postgres.uri.contains("postgres"));
}

/// Filesystem backend uses sensible default path.
#[test]
fn test_filesystem_dlq_config_default() {
    let config = FilesystemDlqConfig::default();
    assert_eq!(config.path, "/var/log/angzarr/dlq");
    assert_eq!(config.format, "json");
    assert_eq!(config.max_files, 0);
}

/// Filesystem offload backend uses different path than regular filesystem.
///
/// Offload stores full protobuf for recovery; regular filesystem stores
/// human-readable JSON for debugging.
#[test]
fn test_offload_filesystem_dlq_config_default() {
    let config = OffloadFilesystemDlqConfig::default();
    assert_eq!(config.base_path, "/var/lib/angzarr/dlq");
    assert_eq!(config.prefix, "dlq/");
}

/// GCS offload backend requires bucket configuration.
#[test]
fn test_offload_gcs_dlq_config_default() {
    let config = OffloadGcsDlqConfig::default();
    assert!(config.bucket.is_empty());
    assert_eq!(config.prefix, "dlq/");
}

/// S3 offload backend requires bucket configuration.
#[test]
fn test_offload_s3_dlq_config_default() {
    let config = OffloadS3DlqConfig::default();
    assert!(config.bucket.is_empty());
    assert_eq!(config.prefix, "dlq/");
    assert!(config.region.is_none());
    assert!(config.endpoint_url.is_none());
}
