//! Edition event store — composite reads from main timeline + edition writes.
//!
//! Wraps an existing `EventStore` to provide edition-aware storage.
//! Edition is a column parameter on all EventStore methods. This store
//! reads main timeline events using `DEFAULT_EDITION` and edition events
//! using `self.edition_name`, then composites them for reads.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::debug;
use uuid::Uuid;

use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::proto::EventPage;
use crate::storage::{EventStore, Result};

use super::metadata::DivergencePoint;

/// Event store that composites main timeline events up to divergence
/// with edition-specific events after divergence.
pub struct EditionEventStore {
    /// Underlying physical store (shared with main timeline).
    inner: Arc<dyn EventStore>,
    /// Edition name (e.g., "v2"), used as the edition column value.
    edition_name: String,
    /// Where this edition diverges from the main timeline.
    divergence: DivergencePoint,
}

impl EditionEventStore {
    /// Create a new edition event store.
    pub fn new(
        inner: Arc<dyn EventStore>,
        edition_name: String,
        divergence: DivergencePoint,
    ) -> Self {
        Self {
            inner,
            edition_name,
            divergence,
        }
    }

    /// Load main timeline events up to the divergence point.
    async fn main_events(&self, domain: &str, root: Uuid) -> Result<Vec<EventPage>> {
        debug!(
            "EditionEventStore::main_events: domain={}, root={}, divergence={:?}",
            domain, root, self.divergence
        );
        let result = match &self.divergence {
            DivergencePoint::AtSequence(seq) => {
                self.inner
                    .get_from_to(domain, DEFAULT_EDITION, root, 0, seq.saturating_add(1))
                    .await
            }
            DivergencePoint::AtTimestamp(ts) => {
                self.inner
                    .get_until_timestamp(domain, DEFAULT_EDITION, root, ts)
                    .await
            }
        };
        debug!("EditionEventStore::main_events: result={:?}", result);
        result
    }

    /// Load edition-specific events (written after divergence).
    async fn edition_events(&self, domain: &str, root: Uuid) -> Result<Vec<EventPage>> {
        debug!(
            "EditionEventStore::edition_events: domain={}, root={}, edition={}",
            domain, root, self.edition_name
        );
        let result = self.inner.get(domain, &self.edition_name, root).await;
        debug!("EditionEventStore::edition_events: result={:?}", result);
        result
    }
}

#[async_trait]
impl EventStore for EditionEventStore {
    /// Store events under the edition's timeline.
    async fn add(
        &self,
        domain: &str,
        _edition: &str,
        root: Uuid,
        events: Vec<EventPage>,
        correlation_id: &str,
    ) -> Result<()> {
        self.inner
            .add(domain, &self.edition_name, root, events, correlation_id)
            .await
    }

    /// Composite read: main events up to divergence + edition events after.
    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Vec<EventPage>> {
        debug!(
            "EditionEventStore::get: domain={}, edition={}, root={}",
            domain, edition, root
        );
        let mut events = self.main_events(domain, root).await?;
        let edition_events = self.edition_events(domain, root).await?;
        events.extend(edition_events);
        debug!("EditionEventStore::get: combined events count={}", events.len());
        Ok(events)
    }

    /// Get events from a sequence onwards (composite).
    async fn get_from(
        &self,
        domain: &str,
        _edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        let all = self.get(domain, "", root).await?;
        Ok(all
            .into_iter()
            .filter(|p| {
                p.sequence
                    .as_ref()
                    .map(|s| match s {
                        crate::proto::event_page::Sequence::Num(n) => *n >= from,
                        _ => true,
                    })
                    .unwrap_or(true)
            })
            .collect())
    }

    /// Get events in range [from, to) (composite).
    async fn get_from_to(
        &self,
        domain: &str,
        _edition: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>> {
        let all = self.get(domain, "", root).await?;
        Ok(all
            .into_iter()
            .filter(|p| {
                p.sequence
                    .as_ref()
                    .map(|s| match s {
                        crate::proto::event_page::Sequence::Num(n) => *n >= from && *n < to,
                        _ => true,
                    })
                    .unwrap_or(true)
            })
            .collect())
    }

    /// List roots: union of main timeline roots and edition roots.
    async fn list_roots(&self, domain: &str, _edition: &str) -> Result<Vec<Uuid>> {
        let main_roots = self.inner.list_roots(domain, DEFAULT_EDITION).await?;
        let edition_roots = self
            .inner
            .list_roots(domain, &self.edition_name)
            .await?;

        let mut all: Vec<Uuid> = main_roots;
        for r in edition_roots {
            if !all.contains(&r) {
                all.push(r);
            }
        }
        Ok(all)
    }

    /// List domains: delegate to inner store.
    async fn list_domains(&self) -> Result<Vec<String>> {
        self.inner.list_domains().await
    }

    /// Next sequence: main divergence count + edition event count.
    async fn get_next_sequence(
        &self,
        domain: &str,
        _edition: &str,
        root: Uuid,
    ) -> Result<u32> {
        let main = self.main_events(domain, root).await?;
        let edition = self.edition_events(domain, root).await?;
        Ok((main.len() + edition.len()) as u32)
    }

    /// Temporal query against the composite view.
    async fn get_until_timestamp(
        &self,
        domain: &str,
        _edition: &str,
        root: Uuid,
        until: &str,
    ) -> Result<Vec<EventPage>> {
        let all = self.get(domain, "", root).await?;
        let until_ts = until.parse::<chrono::DateTime<chrono::Utc>>().ok();
        Ok(all
            .into_iter()
            .filter(|p| {
                if let (Some(created), Some(until)) = (&p.created_at, &until_ts) {
                    let event_ts = chrono::DateTime::from_timestamp(
                        created.seconds,
                        created.nanos as u32,
                    );
                    event_ts.map(|t| t <= *until).unwrap_or(true)
                } else {
                    true
                }
            })
            .collect())
    }

    /// Correlation query: forward to inner store.
    async fn get_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Vec<crate::proto::EventBook>> {
        self.inner.get_by_correlation(correlation_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MockEventStore;

    fn make_event(seq: u32) -> EventPage {
        EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Num(seq)),
            created_at: None,
            event: Some(prost_types::Any {
                type_url: format!("test.Event{}", seq),
                value: vec![],
            }),
        }
    }

    #[tokio::test]
    async fn test_composite_read_combines_main_and_edition() {
        let mock = MockEventStore::new();
        let root = Uuid::new_v4();

        // Main timeline: events 0, 1, 2 under domain "order", edition "angzarr"
        mock.add("order", "angzarr", root, vec![make_event(0), make_event(1), make_event(2)], "")
            .await
            .unwrap();

        // Edition events: 3, 4 under domain "order", edition "v2"
        mock.add("order", "v2", root, vec![make_event(3), make_event(4)], "")
            .await
            .unwrap();

        let store = EditionEventStore::new(
            Arc::new(mock),
            "v2".to_string(),
            DivergencePoint::AtSequence(2),
        );

        let events = store.get("order", "v2", root).await.unwrap();
        assert_eq!(events.len(), 5);
    }

    #[tokio::test]
    async fn test_writes_go_to_edition_domain() {
        let mock = Arc::new(MockEventStore::new());
        let root = Uuid::new_v4();

        let store = EditionEventStore::new(
            mock.clone(),
            "v2".to_string(),
            DivergencePoint::AtSequence(0),
        );

        store
            .add("order", "v2", root, vec![make_event(0)], "corr-1")
            .await
            .unwrap();

        // Should be stored under domain "order", edition "v2"
        let edition_events = mock.get("order", "v2", root).await.unwrap();
        assert_eq!(edition_events.len(), 1);

        // Main edition should be empty for domain "order"
        let main_events = mock.get("order", "angzarr", root).await.unwrap();
        assert!(main_events.is_empty());
    }

    #[tokio::test]
    async fn test_next_sequence_continuation() {
        let mock = MockEventStore::new();
        let root = Uuid::new_v4();

        // Main timeline: 3 events (0, 1, 2)
        mock.add("order", "angzarr", root, vec![make_event(0), make_event(1), make_event(2)], "")
            .await
            .unwrap();

        // Edition: 2 events (3, 4)
        mock.add("order", "v2", root, vec![make_event(3), make_event(4)], "")
            .await
            .unwrap();

        let store = EditionEventStore::new(
            Arc::new(mock),
            "v2".to_string(),
            DivergencePoint::AtSequence(2),
        );

        let next = store.get_next_sequence("order", "v2", root).await.unwrap();
        assert_eq!(next, 5);
    }

    #[tokio::test]
    async fn test_list_roots_union() {
        let mock = MockEventStore::new();
        let root1 = Uuid::new_v4();
        let root2 = Uuid::new_v4();

        mock.add("order", "angzarr", root1, vec![make_event(0)], "")
            .await
            .unwrap();
        mock.add("order", "v2", root2, vec![make_event(0)], "")
            .await
            .unwrap();

        let store = EditionEventStore::new(
            Arc::new(mock),
            "v2".to_string(),
            DivergencePoint::AtSequence(0),
        );

        let roots = store.list_roots("order", "v2").await.unwrap();
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&root1));
        assert!(roots.contains(&root2));
    }

    #[tokio::test]
    async fn test_empty_edition_returns_main_only() {
        let mock = MockEventStore::new();
        let root = Uuid::new_v4();

        mock.add("order", "angzarr", root, vec![make_event(0), make_event(1)], "")
            .await
            .unwrap();

        let store = EditionEventStore::new(
            Arc::new(mock),
            "v2".to_string(),
            DivergencePoint::AtSequence(1),
        );

        let events = store.get("order", "v2", root).await.unwrap();
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn test_get_from_to_composite() {
        let mock = MockEventStore::new();
        let root = Uuid::new_v4();

        mock.add("order", "angzarr", root, vec![make_event(0), make_event(1), make_event(2)], "")
            .await
            .unwrap();
        mock.add("order", "v2", root, vec![make_event(3), make_event(4)], "")
            .await
            .unwrap();

        let store = EditionEventStore::new(
            Arc::new(mock),
            "v2".to_string(),
            DivergencePoint::AtSequence(2),
        );

        // Range [1, 4) should return events 1, 2, 3
        let events = store.get_from_to("order", "v2", root, 1, 4).await.unwrap();
        assert_eq!(events.len(), 3);
    }
}
