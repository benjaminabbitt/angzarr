//! Mock EventStore implementation for testing.

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::proto::{Cover, EventBook, EventPage, Uuid as ProtoUuid};
use crate::storage::{EventStore, Result, StorageError};

/// Stored event with correlation tracking.
struct StoredEvent {
    page: EventPage,
    correlation_id: String,
}

/// Mock event store that stores events in memory.
#[derive(Default)]
pub struct MockEventStore {
    events: RwLock<HashMap<(String, String, Uuid), Vec<StoredEvent>>>,
    fail_on_add: RwLock<bool>,
    fail_on_get: RwLock<bool>,
    next_sequence_override: RwLock<Option<u32>>,
}

impl MockEventStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_fail_on_add(&self, fail: bool) {
        *self.fail_on_add.write().await = fail;
    }

    pub async fn set_fail_on_get(&self, fail: bool) {
        *self.fail_on_get.write().await = fail;
    }

    pub async fn set_next_sequence(&self, seq: u32) {
        *self.next_sequence_override.write().await = Some(seq);
    }

    pub async fn clear_next_sequence_override(&self) {
        *self.next_sequence_override.write().await = None;
    }
}

#[async_trait]
impl EventStore for MockEventStore {
    async fn add(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        events: Vec<EventPage>,
        correlation_id: &str,
    ) -> Result<()> {
        if *self.fail_on_add.read().await {
            return Err(StorageError::NotFound {
                domain: domain.to_string(),
                root,
            });
        }
        let key = (domain.to_string(), edition.to_string(), root);
        let mut store = self.events.write().await;
        let stored: Vec<StoredEvent> = events
            .into_iter()
            .map(|page| StoredEvent {
                page,
                correlation_id: correlation_id.to_string(),
            })
            .collect();
        store.entry(key).or_default().extend(stored);
        Ok(())
    }

    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Vec<EventPage>> {
        if *self.fail_on_get.read().await {
            return Err(StorageError::NotFound {
                domain: domain.to_string(),
                root,
            });
        }
        let key = (domain.to_string(), edition.to_string(), root);
        let store = self.events.read().await;
        Ok(store
            .get(&key)
            .map(|events| events.iter().map(|e| e.page.clone()).collect())
            .unwrap_or_default())
    }

    async fn get_from(&self, domain: &str, edition: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>> {
        let events = self.get(domain, edition, root).await?;
        Ok(events
            .into_iter()
            .filter(|e| {
                if let Some(crate::proto::event_page::Sequence::Num(seq)) = e.sequence {
                    seq >= from
                } else {
                    false
                }
            })
            .collect())
    }

    async fn get_from_to(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>> {
        let events = self.get(domain, edition, root).await?;
        Ok(events
            .into_iter()
            .filter(|e| {
                if let Some(crate::proto::event_page::Sequence::Num(seq)) = e.sequence {
                    seq >= from && seq < to
                } else {
                    false
                }
            })
            .collect())
    }

    async fn get_until_timestamp(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        until: &str,
    ) -> Result<Vec<EventPage>> {
        let events = self.get(domain, edition, root).await?;
        let until_dt = chrono::DateTime::parse_from_rfc3339(until)
            .map_err(|e| StorageError::InvalidTimestampFormat(e.to_string()))?;
        Ok(events
            .into_iter()
            .filter(|e| {
                if let Some(ref ts) = e.created_at {
                    if let Some(dt) = chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
                    {
                        return dt <= until_dt;
                    }
                }
                false
            })
            .collect())
    }

    async fn list_roots(&self, domain: &str, edition: &str) -> Result<Vec<Uuid>> {
        let store = self.events.read().await;
        Ok(store
            .keys()
            .filter(|(d, e, _)| d == domain && e == edition)
            .map(|(_, _, r)| *r)
            .collect())
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        let store = self.events.read().await;
        let mut domains: Vec<_> = store.keys().map(|(d, _, _)| d.clone()).collect();
        domains.sort();
        domains.dedup();
        Ok(domains)
    }

    async fn get_next_sequence(&self, domain: &str, edition: &str, root: Uuid) -> Result<u32> {
        if let Some(seq) = *self.next_sequence_override.read().await {
            return Ok(seq);
        }
        let events = self.get(domain, edition, root).await?;
        Ok(events.len() as u32)
    }

    async fn get_by_correlation(&self, correlation_id: &str) -> Result<Vec<EventBook>> {
        if correlation_id.is_empty() {
            return Ok(vec![]);
        }

        let store = self.events.read().await;
        let mut books_map: HashMap<(String, String, Uuid), Vec<EventPage>> = HashMap::new();

        for ((domain, edition, root), events) in store.iter() {
            for stored in events {
                if stored.correlation_id == correlation_id {
                    books_map
                        .entry((domain.clone(), edition.clone(), *root))
                        .or_default()
                        .push(stored.page.clone());
                }
            }
        }

        let mut books = Vec::with_capacity(books_map.len());
        for ((domain, edition, root), pages) in books_map {
            books.push(EventBook {
                cover: Some(Cover {
                    domain,
                    root: Some(ProtoUuid {
                        value: root.as_bytes().to_vec(),
                    }),
                    correlation_id: correlation_id.to_string(),
                    edition: Some(edition),
                }),
                pages,
                snapshot: None,
                snapshot_state: None,
            });
        }

        Ok(books)
    }
}
