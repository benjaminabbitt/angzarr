//! Tests for TTL-based payload cleanup.
//!
//! TtlReaper runs as a background task deleting expired payloads:
//! - Configurable retention duration
//! - Configurable cleanup interval
//! - run_once() for manual/test invocation
//! - spawn() for background execution
//!
//! Why this matters: Without cleanup, payload storage grows unbounded.
//! Payloads are only needed until consumers process the referencing
//! events, so retention-based cleanup is safe and necessary.
//!
//! Key behaviors verified:
//! - Default interval is 1 hour
//! - with_interval() changes cleanup frequency
//! - run_once() deletes files older than retention
//! - spawn() runs periodic cleanup until aborted
//! - Zero retention deletes everything immediately
//! - Idempotent: repeated runs don't fail on empty store

use super::*;
use crate::payload_store::FilesystemPayloadStore;
use std::time::SystemTime;
use tempfile::TempDir;

// ============================================================================
// Construction Tests
// ============================================================================

/// Default cleanup interval is 1 hour.
#[tokio::test]
async fn test_reaper_new_default_interval() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    let reaper = TtlReaper::new(store, Duration::from_secs(86400)); // 24 hours retention

    // Default interval should be 1 hour (3600 seconds)
    assert_eq!(reaper.interval, Duration::from_secs(3600));
}

/// Retention duration is stored in reaper.
#[tokio::test]
async fn test_reaper_retention_stored() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    let retention = Duration::from_secs(7200); // 2 hours
    let reaper = TtlReaper::new(store, retention);

    assert_eq!(reaper.retention, retention);
}

/// with_interval() sets custom cleanup frequency.
#[tokio::test]
async fn test_reaper_with_custom_interval() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    let reaper =
        TtlReaper::new(store, Duration::from_secs(3600)).with_interval(Duration::from_secs(60));

    assert_eq!(reaper.interval, Duration::from_secs(60));
}

/// Multiple with_interval() calls use last value.
#[tokio::test]
async fn test_reaper_with_interval_chaining() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    let reaper = TtlReaper::new(store, Duration::from_secs(3600))
        .with_interval(Duration::from_secs(60))
        .with_interval(Duration::from_secs(120)); // Second call overwrites

    assert_eq!(reaper.interval, Duration::from_secs(120));
}

// ============================================================================
// Run Once Tests
// ============================================================================

/// run_once() deletes files older than retention.
#[tokio::test]
async fn test_reaper_run_once() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    // Store a payload
    let reference = store.put(b"test payload").await.unwrap();
    let path = std::path::Path::new(reference.uri.strip_prefix("file://").unwrap());

    // Create reaper with 1 hour retention
    let reaper = TtlReaper::new(Arc::clone(&store), Duration::from_secs(3600));

    // Run once - file is new, should not delete
    let deleted = reaper.run_once().await.unwrap();
    assert_eq!(deleted, 0);
    assert!(path.exists());

    // Backdate the file to 2 hours ago
    let old_time =
        std::fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(7200));
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(old_time)
        .unwrap();

    // Run once - file is old, should delete
    let deleted = reaper.run_once().await.unwrap();
    assert_eq!(deleted, 1);
    assert!(!path.exists());
}

/// run_once() on empty store returns 0 deleted.
#[tokio::test]
async fn test_reaper_run_once_empty_store() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    let reaper = TtlReaper::new(store, Duration::from_secs(3600));

    // Run on empty store
    let deleted = reaper.run_once().await.unwrap();
    assert_eq!(deleted, 0);
}

/// run_once() deletes only files older than threshold.
#[tokio::test]
async fn test_reaper_run_once_multiple_files() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    // Store multiple payloads
    let ref1 = store.put(b"payload1").await.unwrap();
    let ref2 = store.put(b"payload2").await.unwrap();
    let ref3 = store.put(b"payload3").await.unwrap();

    let path1 = std::path::Path::new(ref1.uri.strip_prefix("file://").unwrap());
    let path2 = std::path::Path::new(ref2.uri.strip_prefix("file://").unwrap());
    let path3 = std::path::Path::new(ref3.uri.strip_prefix("file://").unwrap());

    let reaper = TtlReaper::new(Arc::clone(&store), Duration::from_secs(3600));

    // Backdate only files 1 and 3
    let old_time =
        std::fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(7200));

    std::fs::File::options()
        .write(true)
        .open(path1)
        .unwrap()
        .set_times(old_time)
        .unwrap();

    std::fs::File::options()
        .write(true)
        .open(path3)
        .unwrap()
        .set_times(old_time)
        .unwrap();

    // Run once - should delete 2 files
    let deleted = reaper.run_once().await.unwrap();
    assert_eq!(deleted, 2);

    // Verify correct files deleted
    assert!(!path1.exists());
    assert!(path2.exists());
    assert!(!path3.exists());
}

/// Boundary condition: file exactly at retention age.
#[tokio::test]
async fn test_reaper_run_once_at_boundary() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    let reference = store.put(b"test payload").await.unwrap();
    let path = std::path::Path::new(reference.uri.strip_prefix("file://").unwrap());

    // Create reaper with exact 1 hour retention
    let reaper = TtlReaper::new(Arc::clone(&store), Duration::from_secs(3600));

    // Backdate file to exactly 1 hour (at the boundary)
    let boundary_time =
        std::fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(3600));
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(boundary_time)
        .unwrap();

    // At exact boundary, behavior depends on implementation
    // Just verify it doesn't panic
    let _deleted = reaper.run_once().await.unwrap();
}

// ============================================================================
// Spawn and Lifecycle Tests
// ============================================================================

/// spawn() returns handle that can be aborted.
#[tokio::test]
async fn test_reaper_spawn_and_abort() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    let reaper =
        TtlReaper::new(store, Duration::from_secs(3600)).with_interval(Duration::from_millis(100)); // Fast interval for testing

    let handle = reaper.spawn();

    // Let it run briefly
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Abort
    handle.abort();

    // Verify it's aborted (should complete quickly)
    let result = tokio::time::timeout(Duration::from_millis(100), handle).await;
    assert!(result.is_ok());
}

/// Spawned reaper actually deletes expired files.
#[tokio::test]
async fn test_reaper_spawn_runs_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    // Store a payload and backdate it
    let reference = store.put(b"test payload").await.unwrap();
    let path = std::path::Path::new(reference.uri.strip_prefix("file://").unwrap());

    let old_time =
        std::fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(7200));
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(old_time)
        .unwrap();

    assert!(path.exists());

    // Create reaper with short interval
    let reaper = TtlReaper::new(Arc::clone(&store), Duration::from_secs(3600))
        .with_interval(Duration::from_millis(50));

    let handle = reaper.spawn();

    // Wait for at least one cleanup cycle
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Verify file was deleted
    assert!(!path.exists());

    handle.abort();
}

/// Multiple cleanup cycles work correctly.
#[tokio::test]
async fn test_reaper_multiple_spawn_cycles() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    let reaper = TtlReaper::new(Arc::clone(&store), Duration::from_secs(3600))
        .with_interval(Duration::from_millis(20));

    let handle = reaper.spawn();

    // Store and backdate payloads in cycles
    for i in 0..3 {
        let reference = store.put(format!("payload{}", i).as_bytes()).await.unwrap();
        let path = std::path::Path::new(reference.uri.strip_prefix("file://").unwrap());

        let old_time =
            std::fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(7200));
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(old_time)
            .unwrap();

        // Wait for cleanup
        tokio::time::sleep(Duration::from_millis(50)).await;

        // File should be cleaned up
        assert!(!path.exists(), "File {} should have been deleted", i);
    }

    handle.abort();
}

// ============================================================================
// Edge Cases
// ============================================================================

/// Zero retention deletes everything immediately.
#[tokio::test]
async fn test_reaper_zero_retention() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    let reference = store.put(b"test payload").await.unwrap();
    let path = std::path::Path::new(reference.uri.strip_prefix("file://").unwrap());

    // Zero retention means delete everything immediately
    let reaper = TtlReaper::new(Arc::clone(&store), Duration::ZERO);

    let deleted = reaper.run_once().await.unwrap();
    assert_eq!(deleted, 1);
    assert!(!path.exists());
}

/// Very long retention (365 days) keeps new files.
#[tokio::test]
async fn test_reaper_very_long_retention() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    let reference = store.put(b"test payload").await.unwrap();
    let path = std::path::Path::new(reference.uri.strip_prefix("file://").unwrap());

    // 365 days retention
    let reaper = TtlReaper::new(Arc::clone(&store), Duration::from_secs(365 * 24 * 3600));

    let deleted = reaper.run_once().await.unwrap();
    assert_eq!(deleted, 0);
    assert!(path.exists());
}

/// Multiple run_once() calls are idempotent.
#[tokio::test]
async fn test_reaper_idempotent_run() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

    let reference = store.put(b"test payload").await.unwrap();
    let path = std::path::Path::new(reference.uri.strip_prefix("file://").unwrap());

    // Backdate file
    let old_time =
        std::fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(7200));
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(old_time)
        .unwrap();

    let reaper = TtlReaper::new(Arc::clone(&store), Duration::from_secs(3600));

    // First run deletes
    let deleted1 = reaper.run_once().await.unwrap();
    assert_eq!(deleted1, 1);

    // Second run finds nothing to delete
    let deleted2 = reaper.run_once().await.unwrap();
    assert_eq!(deleted2, 0);

    // Third run also finds nothing
    let deleted3 = reaper.run_once().await.unwrap();
    assert_eq!(deleted3, 0);
}
