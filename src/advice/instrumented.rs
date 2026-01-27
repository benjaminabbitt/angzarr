//! Metrics instrumentation advice.
//!
//! Wraps storage traits to emit OpenTelemetry-compatible metrics
//! without modifying core implementations.

use std::time::Instant;

use async_trait::async_trait;
use metrics::{counter, histogram};
use uuid::Uuid;

use crate::proto::{EventBook, EventPage, Snapshot};
use crate::storage::{EventStore, PositionStore, Result, SnapshotStore};

/// Wrapper that adds metrics instrumentation to any storage implementation.
///
/// Emits counters and histograms for all operations:
/// - `angzarr_events_stored_total` - Events stored (by domain)
/// - `angzarr_events_loaded_total` - Events loaded (by domain)
/// - `angzarr_snapshots_stored_total` - Snapshots stored (by namespace)
/// - `angzarr_snapshots_loaded_total` - Snapshots loaded (by namespace)
/// - `angzarr_positions_updated_total` - Position updates (by handler)
/// - `angzarr_storage_duration_seconds` - Operation latencies (by operation, storage)
///
/// # Example
///
/// ```ignore
/// let store = SqliteEventStore::new(pool);
/// let store = Instrumented::new(store, "sqlite");
/// ```
pub struct Instrumented<T> {
    inner: T,
    storage_type: &'static str,
}

impl<T> Instrumented<T> {
    /// Wrap a storage implementation with metrics instrumentation.
    ///
    /// # Arguments
    /// * `inner` - The storage implementation to wrap
    /// * `storage_type` - Label for metrics (e.g., "sqlite", "postgres", "redis")
    pub fn new(inner: T, storage_type: &'static str) -> Self {
        Self { inner, storage_type }
    }

    /// Get a reference to the inner storage.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Consume the wrapper and return the inner storage.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

#[async_trait]
impl<T: EventStore> EventStore for Instrumented<T> {
    async fn add(
        &self,
        domain: &str,
        root: Uuid,
        events: Vec<EventPage>,
        correlation_id: &str,
    ) -> Result<()> {
        let start = Instant::now();
        let count = events.len();

        let result = self.inner.add(domain, root, events, correlation_id).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "event_add",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if result.is_ok() {
            counter!(
                "angzarr_events_stored_total",
                "domain" => domain.to_string(),
                "storage" => self.storage_type
            )
            .increment(count as u64);
        }

        result
    }

    async fn get(&self, domain: &str, root: Uuid) -> Result<Vec<EventPage>> {
        let start = Instant::now();

        let result = self.inner.get(domain, root).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "event_get",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if let Ok(ref events) = result {
            counter!(
                "angzarr_events_loaded_total",
                "domain" => domain.to_string(),
                "storage" => self.storage_type
            )
            .increment(events.len() as u64);
        }

        result
    }

    async fn get_from(&self, domain: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>> {
        let start = Instant::now();

        let result = self.inner.get_from(domain, root, from).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "event_get_from",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if let Ok(ref events) = result {
            counter!(
                "angzarr_events_loaded_total",
                "domain" => domain.to_string(),
                "storage" => self.storage_type
            )
            .increment(events.len() as u64);
        }

        result
    }

    async fn get_from_to(
        &self,
        domain: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>> {
        let start = Instant::now();

        let result = self.inner.get_from_to(domain, root, from, to).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "event_get_from_to",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if let Ok(ref events) = result {
            counter!(
                "angzarr_events_loaded_total",
                "domain" => domain.to_string(),
                "storage" => self.storage_type
            )
            .increment(events.len() as u64);
        }

        result
    }

    async fn list_roots(&self, domain: &str) -> Result<Vec<Uuid>> {
        let start = Instant::now();

        let result = self.inner.list_roots(domain).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "event_list_roots",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        result
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        let start = Instant::now();

        let result = self.inner.list_domains().await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "event_list_domains",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        result
    }

    async fn get_next_sequence(&self, domain: &str, root: Uuid) -> Result<u32> {
        let start = Instant::now();

        let result = self.inner.get_next_sequence(domain, root).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "event_get_next_sequence",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        result
    }

    async fn get_by_correlation(&self, correlation_id: &str) -> Result<Vec<EventBook>> {
        let start = Instant::now();

        let result = self.inner.get_by_correlation(correlation_id).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "event_get_by_correlation",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if let Ok(ref books) = result {
            let total_events: usize = books.iter().map(|b| b.pages.len()).sum();
            counter!(
                "angzarr_events_loaded_total",
                "domain" => "correlation_query",
                "storage" => self.storage_type
            )
            .increment(total_events as u64);
        }

        result
    }
}

#[async_trait]
impl<T: SnapshotStore> SnapshotStore for Instrumented<T> {
    async fn get(&self, namespace: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let start = Instant::now();

        let result = self.inner.get(namespace, root).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "snapshot_get",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if let Ok(Some(_)) = &result {
            counter!(
                "angzarr_snapshots_loaded_total",
                "namespace" => namespace.to_string(),
                "storage" => self.storage_type
            )
            .increment(1);
        }

        result
    }

    async fn put(&self, namespace: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let start = Instant::now();

        let result = self.inner.put(namespace, root, snapshot).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "snapshot_put",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if result.is_ok() {
            counter!(
                "angzarr_snapshots_stored_total",
                "namespace" => namespace.to_string(),
                "storage" => self.storage_type
            )
            .increment(1);
        }

        result
    }

    async fn delete(&self, namespace: &str, root: Uuid) -> Result<()> {
        let start = Instant::now();

        let result = self.inner.delete(namespace, root).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "snapshot_delete",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        result
    }
}

#[async_trait]
impl<T: PositionStore> PositionStore for Instrumented<T> {
    async fn get(&self, handler: &str, domain: &str, root: &[u8]) -> Result<Option<u32>> {
        let start = Instant::now();

        let result = self.inner.get(handler, domain, root).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "position_get",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        result
    }

    async fn put(&self, handler: &str, domain: &str, root: &[u8], sequence: u32) -> Result<()> {
        let start = Instant::now();

        let result = self.inner.put(handler, domain, root, sequence).await;

        histogram!(
            "angzarr_storage_duration_seconds",
            "operation" => "position_put",
            "storage" => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if result.is_ok() {
            counter!(
                "angzarr_positions_updated_total",
                "handler" => handler.to_string(),
                "domain" => domain.to_string(),
                "storage" => self.storage_type
            )
            .increment(1);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MockEventStore;

    #[tokio::test]
    async fn test_instrumented_delegates_to_inner() {
        let inner = MockEventStore::new();
        let instrumented = Instrumented::new(inner, "mock");

        let root = Uuid::new_v4();

        // Should delegate and succeed
        let events = instrumented.get("test", root).await.unwrap();
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_instrumented_preserves_errors() {
        let inner = MockEventStore::new();
        inner.set_fail_on_get(true).await;

        let instrumented = Instrumented::new(inner, "mock");
        let root = Uuid::new_v4();

        // Should propagate error
        let result = instrumented.get("test", root).await;
        assert!(result.is_err());
    }
}
