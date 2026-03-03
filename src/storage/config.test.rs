//! Tests for storage configuration types.
//!
//! Storage config is a discriminated union supporting multiple backends:
//! PostgreSQL (default), SQLite, Redis, Bigtable, DynamoDB, NATS.
//! Each backend has its own sub-configuration.
//!
//! Why this matters: Different deployments need different storage backends.
//! Development uses SQLite (zero setup), production uses PostgreSQL
//! (ACID guarantees). Config validation prevents runtime surprises.
//!
//! Key behaviors verified:
//! - Default storage type is postgres (production-ready default)
//! - SQLite URI handling (in-memory vs file paths)
//! - Snapshot enable flags for debugging/troubleshooting

use super::*;

// ============================================================================
// StorageConfig Tests
// ============================================================================
//
// The storage config is a discriminated union with `storage_type` as the
// discriminator. All backend configs are always present (for deserialization),
// but only the one matching `storage_type` is actually used.

/// Default storage config targets PostgreSQL with standard connection string.
///
/// PostgreSQL is the default because it's the most commonly deployed
/// production database with full ACID guarantees.
#[test]
fn test_storage_config_default() {
    let storage = StorageConfig::default();
    assert_eq!(storage.storage_type, "postgres");
    assert_eq!(storage.postgres.uri, "postgres://localhost:5432/angzarr");
    assert!(storage.snapshots_enable.read);
    assert!(storage.snapshots_enable.write);
}

// ============================================================================
// SqliteConfig Tests
// ============================================================================
//
// SQLite is the development backend. The URI generation handles the edge
// case of in-memory vs file paths correctly.

/// Default SQLite config uses in-memory database.
///
/// In-memory is safest for testing—no file cleanup needed.
/// Production deployments should configure explicit path.
#[test]
fn test_sqlite_uri_memory() {
    let config = SqliteConfig::default();
    assert_eq!(config.uri(), "sqlite::memory:");
}

/// File path SQLite config generates correct URI.
#[test]
fn test_sqlite_uri_file() {
    let config = SqliteConfig {
        path: Some("/tmp/test.db".to_string()),
    };
    assert_eq!(config.uri(), "sqlite:/tmp/test.db");
}

/// Empty path string treated as in-memory (not empty file path).
///
/// Edge case: config deserialization may produce `Some("")` rather than
/// `None`. The uri() method treats this as in-memory to avoid creating
/// a database at the current directory with no name.
#[test]
fn test_sqlite_uri_empty_string_is_memory() {
    let config = SqliteConfig {
        path: Some(String::new()),
    };
    assert_eq!(config.uri(), "sqlite::memory:");
}

// ============================================================================
// SnapshotsEnableConfig Tests
// ============================================================================
//
// Snapshot flags control read/write of snapshots. Disabling is useful for
// debugging:
// - read=false: Force full event replay (verify replay correctness)
// - write=false: Pure event sourcing mode (no snapshot storage)

/// Default snapshot config enables both read and write.
///
/// Snapshots improve performance by avoiding full event replay.
/// Both flags are enabled by default; disable for debugging:
/// - read=false: Force full event replay (verify replay correctness)
/// - write=false: Pure event sourcing mode (no snapshot storage)
#[test]
fn test_snapshots_enable_config_default() {
    let config = SnapshotsEnableConfig::default();
    assert!(config.read);
    assert!(config.write);
}
