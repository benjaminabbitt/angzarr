//! TTL-based cleanup for expired payloads.
//!
//! The `TtlReaper` runs as a background task in coordinators, periodically
//! cleaning up payloads that have exceeded their retention period.

use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::{info, warn};

use super::PayloadStore;

/// Background task for cleaning up expired payloads.
///
/// Runs periodically and deletes payloads older than the configured retention.
pub struct TtlReaper<S: PayloadStore> {
    store: Arc<S>,
    retention: Duration,
    interval: Duration,
}

impl<S: PayloadStore + 'static> TtlReaper<S> {
    /// Create a new TTL reaper.
    ///
    /// # Arguments
    /// * `store` - The payload store to clean up
    /// * `retention` - Maximum age for payloads (older ones are deleted)
    /// * `interval` - How often to run cleanup (default: every hour)
    pub fn new(store: Arc<S>, retention: Duration) -> Self {
        Self {
            store,
            retention,
            interval: Duration::from_secs(3600), // Default: hourly
        }
    }

    /// Set custom cleanup interval.
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Spawn the reaper as a background task.
    ///
    /// Returns a handle that can be used to abort the task.
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                match self.store.delete_older_than(self.retention).await {
                    Ok(count) if count > 0 => {
                        info!(
                            deleted = count,
                            retention_hours = self.retention.as_secs() / 3600,
                            "TTL reaper cleaned up expired payloads"
                        );
                    }
                    Ok(_) => {
                        // Nothing to delete, don't log
                    }
                    Err(e) => {
                        warn!(error = %e, "TTL reaper failed to clean up payloads");
                    }
                }
            }
        })
    }

    /// Run cleanup once (for testing or manual invocation).
    pub async fn run_once(&self) -> super::Result<usize> {
        self.store.delete_older_than(self.retention).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload_store::FilesystemPayloadStore;
    use std::time::SystemTime;
    use tempfile::TempDir;

    // ============================================================================
    // Construction Tests
    // ============================================================================

    #[tokio::test]
    async fn test_reaper_new_default_interval() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

        let reaper = TtlReaper::new(store, Duration::from_secs(86400)); // 24 hours retention

        // Default interval should be 1 hour (3600 seconds)
        assert_eq!(reaper.interval, Duration::from_secs(3600));
    }

    #[tokio::test]
    async fn test_reaper_retention_stored() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

        let retention = Duration::from_secs(7200); // 2 hours
        let reaper = TtlReaper::new(store, retention);

        assert_eq!(reaper.retention, retention);
    }

    #[tokio::test]
    async fn test_reaper_with_custom_interval() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

        let reaper =
            TtlReaper::new(store, Duration::from_secs(3600)).with_interval(Duration::from_secs(60));

        assert_eq!(reaper.interval, Duration::from_secs(60));
    }

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

    #[tokio::test]
    async fn test_reaper_run_once_empty_store() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

        let reaper = TtlReaper::new(store, Duration::from_secs(3600));

        // Run on empty store
        let deleted = reaper.run_once().await.unwrap();
        assert_eq!(deleted, 0);
    }

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

    #[tokio::test]
    async fn test_reaper_spawn_and_abort() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

        let reaper = TtlReaper::new(store, Duration::from_secs(3600))
            .with_interval(Duration::from_millis(100)); // Fast interval for testing

        let handle = reaper.spawn();

        // Let it run briefly
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Abort
        handle.abort();

        // Verify it's aborted (should complete quickly)
        let result = tokio::time::timeout(Duration::from_millis(100), handle).await;
        assert!(result.is_ok());
    }

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

            let old_time = std::fs::FileTimes::new()
                .set_modified(SystemTime::now() - Duration::from_secs(7200));
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
}
