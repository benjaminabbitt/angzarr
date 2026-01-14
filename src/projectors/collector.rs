//! Collector projector for testing.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::interfaces::projector::{Projector, Result};
use crate::proto::{EventBook, Projection};

/// Projector that collects all received events for later inspection.
///
/// Useful for testing to verify which events were published.
pub struct CollectorProjector {
    name: String,
    domains: Vec<String>,
    collected: Arc<RwLock<Vec<EventBook>>>,
}

impl CollectorProjector {
    /// Create a new collector projector.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            domains: Vec::new(),
            collected: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a collector projector for specific domains.
    pub fn for_domains(name: impl Into<String>, domains: Vec<String>) -> Self {
        Self {
            name: name.into(),
            domains,
            collected: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get a handle to the collected events.
    ///
    /// This can be cloned and used to inspect events from tests.
    pub fn collected(&self) -> Arc<RwLock<Vec<EventBook>>> {
        Arc::clone(&self.collected)
    }

    /// Get the number of collected event books.
    pub async fn count(&self) -> usize {
        self.collected.read().await.len()
    }

    /// Get the total number of individual events collected.
    pub async fn event_count(&self) -> usize {
        self.collected
            .read()
            .await
            .iter()
            .map(|b| b.pages.len())
            .sum()
    }

    /// Clear all collected events.
    pub async fn clear(&self) {
        self.collected.write().await.clear();
    }

    /// Take all collected events, leaving the collector empty.
    pub async fn take(&self) -> Vec<EventBook> {
        std::mem::take(&mut *self.collected.write().await)
    }
}

#[async_trait]
impl Projector for CollectorProjector {
    fn name(&self) -> &str {
        &self.name
    }

    fn domains(&self) -> Vec<String> {
        self.domains.clone()
    }

    async fn project(&self, book: &Arc<EventBook>) -> Result<Option<Projection>> {
        self.collected.write().await.push((**book).clone());
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{event_page, Cover, EventPage, Uuid as ProtoUuid};
    use prost_types::Any;

    fn make_event_book(domain: &str, event_count: usize) -> EventBook {
        let root = ProtoUuid {
            value: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        };

        let pages: Vec<EventPage> = (0..event_count)
            .map(|i| EventPage {
                sequence: Some(event_page::Sequence::Num(i as u32)),
                event: Some(Any {
                    type_url: format!("test.Event{}", i),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            })
            .collect();

        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(root),
            }),
            pages,
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
    }

    #[tokio::test]
    async fn test_collector_collects_events() {
        let projector = CollectorProjector::new("test_collector");

        projector
            .project(&Arc::new(make_event_book("orders", 2)))
            .await
            .unwrap();
        projector
            .project(&Arc::new(make_event_book("orders", 3)))
            .await
            .unwrap();

        assert_eq!(projector.count().await, 2);
        assert_eq!(projector.event_count().await, 5);
    }

    #[tokio::test]
    async fn test_collector_take_clears() {
        let projector = CollectorProjector::new("test_collector");

        projector
            .project(&Arc::new(make_event_book("orders", 2)))
            .await
            .unwrap();

        let taken = projector.take().await;
        assert_eq!(taken.len(), 1);
        assert_eq!(projector.count().await, 0);
    }

    #[tokio::test]
    async fn test_collector_shared_handle() {
        let projector = CollectorProjector::new("test_collector");
        let handle = projector.collected();

        projector
            .project(&Arc::new(make_event_book("orders", 2)))
            .await
            .unwrap();

        assert_eq!(handle.read().await.len(), 1);
    }

    #[tokio::test]
    async fn test_collector_for_domains() {
        let projector =
            CollectorProjector::for_domains("domain_collector", vec!["orders".to_string()]);

        assert_eq!(projector.name(), "domain_collector");
        assert_eq!(projector.domains(), vec!["orders".to_string()]);
    }

    #[tokio::test]
    async fn test_collector_clear() {
        let projector = CollectorProjector::new("test_collector");

        projector
            .project(&Arc::new(make_event_book("orders", 2)))
            .await
            .unwrap();
        projector
            .project(&Arc::new(make_event_book("orders", 3)))
            .await
            .unwrap();

        assert_eq!(projector.count().await, 2);

        projector.clear().await;

        assert_eq!(projector.count().await, 0);
    }

    #[tokio::test]
    async fn test_collector_name_and_domains() {
        let projector = CollectorProjector::new("my_collector");

        assert_eq!(projector.name(), "my_collector");
        assert!(projector.domains().is_empty());
    }
}
