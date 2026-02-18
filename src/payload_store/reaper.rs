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
    async fn test_reaper_with_custom_interval() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

        let reaper =
            TtlReaper::new(store, Duration::from_secs(3600)).with_interval(Duration::from_secs(60));

        assert_eq!(reaper.interval, Duration::from_secs(60));
    }

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
}
