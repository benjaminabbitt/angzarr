//! Metrics instrumentation advice.
//!
//! Wraps storage traits to emit OpenTelemetry metrics without modifying
//! core implementations. When the `otel` feature is disabled, the wrapper
//! passes through calls without any overhead.
//!
//! # Example
//!
//! ```ignore
//! let store = SqliteEventStore::new(pool);
//! let store = Instrumented::new(store, "sqlite");
//! // All operations now emit metrics automatically
//! ```

use std::time::Instant;

use async_trait::async_trait;
use uuid::Uuid;

use crate::proto::{EventBook, EventPage, Snapshot};
use crate::storage::{EventStore, PositionStore, Result, SnapshotStore};

// Re-export constants for backwards compatibility
#[allow(unused_imports)]
pub use super::metrics::{
    DOMAIN_CORRELATION_QUERY, OP_EVENT_ADD, OP_EVENT_DELETE_EDITION, OP_EVENT_GET,
    OP_EVENT_GET_BY_CORRELATION, OP_EVENT_GET_FROM, OP_EVENT_GET_FROM_TO,
    OP_EVENT_GET_NEXT_SEQUENCE, OP_EVENT_GET_UNTIL_TIMESTAMP, OP_EVENT_LIST_DOMAINS,
    OP_EVENT_LIST_ROOTS, OP_POSITION_GET, OP_POSITION_PUT, OP_SNAPSHOT_DELETE, OP_SNAPSHOT_GET,
    OP_SNAPSHOT_GET_AT_SEQ, OP_SNAPSHOT_PUT,
};

// OTel metric instruments and helpers (only when otel feature enabled)
#[cfg(feature = "otel")]
use super::metrics::{
    domain_attr, handler_attr, operation_attr, storage_type_attr, EVENTS_LOADED_TOTAL,
    EVENTS_STORED_TOTAL, POSITIONS_UPDATED_TOTAL, SNAPSHOTS_LOADED_TOTAL, SNAPSHOTS_STORED_TOTAL,
    STORAGE_DURATION,
};

/// Wrapper that adds metrics instrumentation to any storage implementation.
///
/// When the `otel` feature is enabled, emits histograms and counters for all
/// operations. When disabled, passes through to the inner implementation
/// with no overhead.
pub struct Instrumented<T> {
    inner: T,
    #[allow(dead_code)] // Used when otel feature enabled
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
        edition: &str,
        root: Uuid,
        events: Vec<EventPage>,
        correlation_id: &str,
    ) -> Result<()> {
        let start = Instant::now();
        let count = events.len();

        let result = self
            .inner
            .add(domain, edition, root, events, correlation_id)
            .await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_EVENT_ADD),
                    storage_type_attr(self.storage_type),
                ],
            );

            if result.is_ok() {
                EVENTS_STORED_TOTAL.add(
                    count as u64,
                    &[domain_attr(domain), storage_type_attr(self.storage_type)],
                );
            }
        }
        let _ = (start, count); // Suppress unused warnings when otel disabled

        result
    }

    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Vec<EventPage>> {
        let start = Instant::now();

        let result = self.inner.get(domain, edition, root).await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_EVENT_GET),
                    storage_type_attr(self.storage_type),
                ],
            );

            if let Ok(ref events) = result {
                EVENTS_LOADED_TOTAL.add(
                    events.len() as u64,
                    &[domain_attr(domain), storage_type_attr(self.storage_type)],
                );
            }
        }
        let _ = start;

        result
    }

    async fn get_from(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        let start = Instant::now();

        let result = self.inner.get_from(domain, edition, root, from).await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_EVENT_GET_FROM),
                    storage_type_attr(self.storage_type),
                ],
            );

            if let Ok(ref events) = result {
                EVENTS_LOADED_TOTAL.add(
                    events.len() as u64,
                    &[domain_attr(domain), storage_type_attr(self.storage_type)],
                );
            }
        }
        let _ = start;

        result
    }

    async fn get_from_to(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>> {
        let start = Instant::now();

        let result = self
            .inner
            .get_from_to(domain, edition, root, from, to)
            .await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_EVENT_GET_FROM_TO),
                    storage_type_attr(self.storage_type),
                ],
            );

            if let Ok(ref events) = result {
                EVENTS_LOADED_TOTAL.add(
                    events.len() as u64,
                    &[domain_attr(domain), storage_type_attr(self.storage_type)],
                );
            }
        }
        let _ = start;

        result
    }

    async fn list_roots(&self, domain: &str, edition: &str) -> Result<Vec<Uuid>> {
        let start = Instant::now();

        let result = self.inner.list_roots(domain, edition).await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_EVENT_LIST_ROOTS),
                    storage_type_attr(self.storage_type),
                ],
            );
        }
        let _ = start;

        result
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        let start = Instant::now();

        let result = self.inner.list_domains().await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_EVENT_LIST_DOMAINS),
                    storage_type_attr(self.storage_type),
                ],
            );
        }
        let _ = start;

        result
    }

    async fn get_next_sequence(&self, domain: &str, edition: &str, root: Uuid) -> Result<u32> {
        let start = Instant::now();

        let result = self.inner.get_next_sequence(domain, edition, root).await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_EVENT_GET_NEXT_SEQUENCE),
                    storage_type_attr(self.storage_type),
                ],
            );
        }
        let _ = start;

        result
    }

    async fn get_by_correlation(&self, correlation_id: &str) -> Result<Vec<EventBook>> {
        let start = Instant::now();

        let result = self.inner.get_by_correlation(correlation_id).await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_EVENT_GET_BY_CORRELATION),
                    storage_type_attr(self.storage_type),
                ],
            );

            if let Ok(ref books) = result {
                let total_events: usize = books.iter().map(|b| b.pages.len()).sum();
                EVENTS_LOADED_TOTAL.add(
                    total_events as u64,
                    &[
                        domain_attr(DOMAIN_CORRELATION_QUERY),
                        storage_type_attr(self.storage_type),
                    ],
                );
            }
        }
        let _ = start;

        result
    }

    async fn get_until_timestamp(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        until: &str,
    ) -> Result<Vec<EventPage>> {
        let start = Instant::now();

        let result = self
            .inner
            .get_until_timestamp(domain, edition, root, until)
            .await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_EVENT_GET_UNTIL_TIMESTAMP),
                    storage_type_attr(self.storage_type),
                ],
            );

            if let Ok(ref events) = result {
                EVENTS_LOADED_TOTAL.add(
                    events.len() as u64,
                    &[domain_attr(domain), storage_type_attr(self.storage_type)],
                );
            }
        }
        let _ = start;

        result
    }

    async fn delete_edition_events(&self, domain: &str, edition: &str) -> Result<u32> {
        let start = Instant::now();

        let result = self.inner.delete_edition_events(domain, edition).await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_EVENT_DELETE_EDITION),
                    storage_type_attr(self.storage_type),
                ],
            );
        }
        let _ = start;

        result
    }
}

#[async_trait]
impl<T: SnapshotStore> SnapshotStore for Instrumented<T> {
    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let start = Instant::now();

        let result = self.inner.get(domain, edition, root).await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_SNAPSHOT_GET),
                    storage_type_attr(self.storage_type),
                ],
            );

            if let Ok(Some(_)) = &result {
                SNAPSHOTS_LOADED_TOTAL.add(
                    1,
                    &[domain_attr(domain), storage_type_attr(self.storage_type)],
                );
            }
        }
        let _ = start;

        result
    }

    async fn get_at_seq(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        seq: u32,
    ) -> Result<Option<Snapshot>> {
        let start = Instant::now();

        let result = self.inner.get_at_seq(domain, edition, root, seq).await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_SNAPSHOT_GET_AT_SEQ),
                    storage_type_attr(self.storage_type),
                ],
            );

            if let Ok(Some(_)) = &result {
                SNAPSHOTS_LOADED_TOTAL.add(
                    1,
                    &[domain_attr(domain), storage_type_attr(self.storage_type)],
                );
            }
        }
        let _ = start;

        result
    }

    async fn put(&self, domain: &str, edition: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let start = Instant::now();

        let result = self.inner.put(domain, edition, root, snapshot).await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_SNAPSHOT_PUT),
                    storage_type_attr(self.storage_type),
                ],
            );

            if result.is_ok() {
                SNAPSHOTS_STORED_TOTAL.add(
                    1,
                    &[domain_attr(domain), storage_type_attr(self.storage_type)],
                );
            }
        }
        let _ = start;

        result
    }

    async fn delete(&self, domain: &str, edition: &str, root: Uuid) -> Result<()> {
        let start = Instant::now();

        let result = self.inner.delete(domain, edition, root).await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_SNAPSHOT_DELETE),
                    storage_type_attr(self.storage_type),
                ],
            );
        }
        let _ = start;

        result
    }
}

#[async_trait]
impl<T: PositionStore> PositionStore for Instrumented<T> {
    async fn get(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
    ) -> Result<Option<u32>> {
        let start = Instant::now();

        let result = self.inner.get(handler, domain, edition, root).await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_POSITION_GET),
                    storage_type_attr(self.storage_type),
                ],
            );
        }
        let _ = start;

        result
    }

    async fn put(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
        sequence: u32,
    ) -> Result<()> {
        let start = Instant::now();

        let result = self
            .inner
            .put(handler, domain, edition, root, sequence)
            .await;

        #[cfg(feature = "otel")]
        {
            STORAGE_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    operation_attr(OP_POSITION_PUT),
                    storage_type_attr(self.storage_type),
                ],
            );

            if result.is_ok() {
                POSITIONS_UPDATED_TOTAL.add(
                    1,
                    &[
                        handler_attr(handler),
                        domain_attr(domain),
                        storage_type_attr(self.storage_type),
                    ],
                );
            }
        }
        let _ = start;

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
        let events = instrumented.get("test", "angzarr", root).await.unwrap();
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_instrumented_preserves_errors() {
        let inner = MockEventStore::new();
        inner.set_fail_on_get(true).await;

        let instrumented = Instrumented::new(inner, "mock");
        let root = Uuid::new_v4();

        // Should propagate error
        let result = instrumented.get("test", "angzarr", root).await;
        assert!(result.is_err());
    }
}
