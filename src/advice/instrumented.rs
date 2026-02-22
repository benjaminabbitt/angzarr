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

// =============================================================================
// Metric Names
// =============================================================================

/// Histogram tracking storage operation latencies.
pub const METRIC_STORAGE_DURATION: &str = "angzarr_storage_duration_seconds";
/// Counter for events stored by domain.
pub const METRIC_EVENTS_STORED: &str = "angzarr_events_stored_total";
/// Counter for events loaded by domain.
pub const METRIC_EVENTS_LOADED: &str = "angzarr_events_loaded_total";
/// Counter for snapshots stored by namespace.
pub const METRIC_SNAPSHOTS_STORED: &str = "angzarr_snapshots_stored_total";
/// Counter for snapshots loaded by namespace.
pub const METRIC_SNAPSHOTS_LOADED: &str = "angzarr_snapshots_loaded_total";
/// Counter for position updates by handler.
pub const METRIC_POSITIONS_UPDATED: &str = "angzarr_positions_updated_total";

// =============================================================================
// Label Names
// =============================================================================

/// Label for operation type (e.g., "event_add", "snapshot_get").
pub const LABEL_OPERATION: &str = "operation";
/// Label for storage backend type (e.g., "sqlite", "postgres").
pub const LABEL_STORAGE: &str = "storage";
/// Label for domain name.
pub const LABEL_DOMAIN: &str = "domain";
/// Label for snapshot namespace.
pub const LABEL_NAMESPACE: &str = "namespace";
/// Label for handler name.
pub const LABEL_HANDLER: &str = "handler";

// =============================================================================
// Operation Names
// =============================================================================

/// Operation: add events to store.
pub const OP_EVENT_ADD: &str = "event_add";
/// Operation: get events by root.
pub const OP_EVENT_GET: &str = "event_get";
/// Operation: get events from sequence.
pub const OP_EVENT_GET_FROM: &str = "event_get_from";
/// Operation: get events in sequence range.
pub const OP_EVENT_GET_FROM_TO: &str = "event_get_from_to";
/// Operation: list aggregate roots.
pub const OP_EVENT_LIST_ROOTS: &str = "event_list_roots";
/// Operation: list domains.
pub const OP_EVENT_LIST_DOMAINS: &str = "event_list_domains";
/// Operation: get next sequence number.
pub const OP_EVENT_GET_NEXT_SEQUENCE: &str = "event_get_next_sequence";
/// Operation: get events by correlation ID.
pub const OP_EVENT_GET_BY_CORRELATION: &str = "event_get_by_correlation";
/// Operation: get snapshot.
pub const OP_SNAPSHOT_GET: &str = "snapshot_get";
/// Operation: put snapshot.
pub const OP_SNAPSHOT_PUT: &str = "snapshot_put";
/// Operation: delete snapshot.
pub const OP_SNAPSHOT_DELETE: &str = "snapshot_delete";
/// Operation: get position.
pub const OP_POSITION_GET: &str = "position_get";
/// Operation: put position.
pub const OP_POSITION_PUT: &str = "position_put";

/// Placeholder domain for correlation queries spanning multiple domains.
pub const DOMAIN_CORRELATION_QUERY: &str = "correlation_query";

/// Wrapper that adds metrics instrumentation to any storage implementation.
///
/// Emits counters and histograms for all operations:
/// - [`METRIC_EVENTS_STORED`] - Events stored (by domain)
/// - [`METRIC_EVENTS_LOADED`] - Events loaded (by domain)
/// - [`METRIC_SNAPSHOTS_STORED`] - Snapshots stored (by namespace)
/// - [`METRIC_SNAPSHOTS_LOADED`] - Snapshots loaded (by namespace)
/// - [`METRIC_POSITIONS_UPDATED`] - Position updates (by handler)
/// - [`METRIC_STORAGE_DURATION`] - Operation latencies (by operation, storage)
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
        Self {
            inner,
            storage_type,
        }
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
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_EVENT_ADD,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if result.is_ok() {
            counter!(
                METRIC_EVENTS_STORED,
                LABEL_DOMAIN => domain.to_string(),
                LABEL_STORAGE => self.storage_type
            )
            .increment(count as u64);
        }

        result
    }

    async fn get(&self, domain: &str, root: Uuid) -> Result<Vec<EventPage>> {
        let start = Instant::now();

        let result = self.inner.get(domain, root).await;

        histogram!(
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_EVENT_GET,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if let Ok(ref events) = result {
            counter!(
                METRIC_EVENTS_LOADED,
                LABEL_DOMAIN => domain.to_string(),
                LABEL_STORAGE => self.storage_type
            )
            .increment(events.len() as u64);
        }

        result
    }

    async fn get_from(&self, domain: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>> {
        let start = Instant::now();

        let result = self.inner.get_from(domain, root, from).await;

        histogram!(
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_EVENT_GET_FROM,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if let Ok(ref events) = result {
            counter!(
                METRIC_EVENTS_LOADED,
                LABEL_DOMAIN => domain.to_string(),
                LABEL_STORAGE => self.storage_type
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
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_EVENT_GET_FROM_TO,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if let Ok(ref events) = result {
            counter!(
                METRIC_EVENTS_LOADED,
                LABEL_DOMAIN => domain.to_string(),
                LABEL_STORAGE => self.storage_type
            )
            .increment(events.len() as u64);
        }

        result
    }

    async fn list_roots(&self, domain: &str) -> Result<Vec<Uuid>> {
        let start = Instant::now();

        let result = self.inner.list_roots(domain).await;

        histogram!(
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_EVENT_LIST_ROOTS,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        result
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        let start = Instant::now();

        let result = self.inner.list_domains().await;

        histogram!(
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_EVENT_LIST_DOMAINS,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        result
    }

    async fn get_next_sequence(&self, domain: &str, root: Uuid) -> Result<u32> {
        let start = Instant::now();

        let result = self.inner.get_next_sequence(domain, root).await;

        histogram!(
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_EVENT_GET_NEXT_SEQUENCE,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        result
    }

    async fn get_by_correlation(&self, correlation_id: &str) -> Result<Vec<EventBook>> {
        let start = Instant::now();

        let result = self.inner.get_by_correlation(correlation_id).await;

        histogram!(
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_EVENT_GET_BY_CORRELATION,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if let Ok(ref books) = result {
            let total_events: usize = books.iter().map(|b| b.pages.len()).sum();
            counter!(
                METRIC_EVENTS_LOADED,
                LABEL_DOMAIN => DOMAIN_CORRELATION_QUERY,
                LABEL_STORAGE => self.storage_type
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
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_SNAPSHOT_GET,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if let Ok(Some(_)) = &result {
            counter!(
                METRIC_SNAPSHOTS_LOADED,
                LABEL_NAMESPACE => namespace.to_string(),
                LABEL_STORAGE => self.storage_type
            )
            .increment(1);
        }

        result
    }

    async fn put(&self, namespace: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let start = Instant::now();

        let result = self.inner.put(namespace, root, snapshot).await;

        histogram!(
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_SNAPSHOT_PUT,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if result.is_ok() {
            counter!(
                METRIC_SNAPSHOTS_STORED,
                LABEL_NAMESPACE => namespace.to_string(),
                LABEL_STORAGE => self.storage_type
            )
            .increment(1);
        }

        result
    }

    async fn delete(&self, namespace: &str, root: Uuid) -> Result<()> {
        let start = Instant::now();

        let result = self.inner.delete(namespace, root).await;

        histogram!(
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_SNAPSHOT_DELETE,
            LABEL_STORAGE => self.storage_type
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
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_POSITION_GET,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        result
    }

    async fn put(&self, handler: &str, domain: &str, root: &[u8], sequence: u32) -> Result<()> {
        let start = Instant::now();

        let result = self.inner.put(handler, domain, root, sequence).await;

        histogram!(
            METRIC_STORAGE_DURATION,
            LABEL_OPERATION => OP_POSITION_PUT,
            LABEL_STORAGE => self.storage_type
        )
        .record(start.elapsed().as_secs_f64());

        if result.is_ok() {
            counter!(
                METRIC_POSITIONS_UPDATED,
                LABEL_HANDLER => handler.to_string(),
                LABEL_DOMAIN => domain.to_string(),
                LABEL_STORAGE => self.storage_type
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
