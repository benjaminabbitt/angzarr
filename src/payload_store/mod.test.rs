//! Tests for payload store core functionality.
//!
//! The payload store implements the claim check pattern: large payloads
//! are stored externally and replaced with references. Content-addressable
//! storage (SHA-256 hashing) enables deduplication and integrity verification.
//!
//! Why this matters: Message buses have size limits (SQS: 256KB, Kafka: 1MB).
//! Without payload offloading, large events/commands would fail to transmit.
//! The hash-based storage also prevents storing duplicate payloads.
//!
//! Key behaviors verified:
//! - Hash computation is deterministic (same input → same hash)
//! - Hash hex roundtrip preserves data
//! - Factory respects enabled/disabled config
//! - Store + retrieve roundtrip works correctly

use super::*;
use tempfile::TempDir;

// ============================================================================
// Hash Function Tests
// ============================================================================

/// Same payload always produces the same hash.
///
/// This is foundational for content-addressable storage and deduplication.
#[test]
fn test_compute_hash_deterministic() {
    let payload = b"test payload data";
    let hash1 = compute_hash(payload);
    let hash2 = compute_hash(payload);
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 32); // SHA-256 produces 32 bytes
}

/// Different payloads produce different hashes.
///
/// Collision resistance: distinct payloads get distinct storage keys.
#[test]
fn test_compute_hash_different_inputs() {
    let hash1 = compute_hash(b"payload 1");
    let hash2 = compute_hash(b"payload 2");
    assert_ne!(hash1, hash2);
}

/// Hash can be converted to hex and back without loss.
///
/// Hex encoding is used for filenames and URIs.
#[test]
fn test_hash_hex_roundtrip() {
    let original = compute_hash(b"test");
    let hex = hash_to_hex(&original);
    let recovered = hex_to_hash(&hex).unwrap();
    assert_eq!(original, recovered);
}

/// Invalid hex string returns error.
#[test]
fn test_hex_to_hash_invalid() {
    let result = hex_to_hash("not valid hex!");
    assert!(result.is_err());
}

// ============================================================================
// Factory Tests
// ============================================================================

/// Factory returns None when offloading is disabled.
///
/// Disabled config means no external storage needed.
#[tokio::test]
async fn test_init_payload_store_disabled() {
    let config = PayloadOffloadConfig {
        enabled: false,
        ..Default::default()
    };

    let result = init_payload_store(&config).await.unwrap();
    assert!(result.is_none());
}

/// Factory creates filesystem store when configured.
#[tokio::test]
async fn test_init_payload_store_filesystem() {
    let temp_dir = TempDir::new().unwrap();
    let config = PayloadOffloadConfig {
        enabled: true,
        store_type: PayloadStoreType::Filesystem,
        filesystem: FilesystemStoreConfig {
            base_path: temp_dir.path().to_path_buf(),
        },
        ..Default::default()
    };

    let result = init_payload_store(&config).await.unwrap();
    assert!(result.is_some());

    let store = result.unwrap();
    assert_eq!(
        store.storage_type(),
        crate::proto::PayloadStorageType::Filesystem
    );
}

/// Full store + retrieve roundtrip works correctly.
///
/// End-to-end test: payload → store → reference → retrieve → payload.
#[tokio::test]
async fn test_init_payload_store_can_store_and_retrieve() {
    let temp_dir = TempDir::new().unwrap();
    let config = PayloadOffloadConfig {
        enabled: true,
        store_type: PayloadStoreType::Filesystem,
        filesystem: FilesystemStoreConfig {
            base_path: temp_dir.path().to_path_buf(),
        },
        ..Default::default()
    };

    let store = init_payload_store(&config).await.unwrap().unwrap();

    let payload = b"test payload from factory";
    let reference = store.put(payload).await.unwrap();
    let retrieved = store.get(&reference).await.unwrap();

    assert_eq!(payload.as_slice(), retrieved.as_slice());
}
