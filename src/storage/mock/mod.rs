//! Mock storage implementations for testing.

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{EventStore, Result, SnapshotStore, StorageError};
use crate::proto::{Cover, EventBook, EventPage, Snapshot, Uuid as ProtoUuid};

/// Stored event with correlation tracking.
struct StoredEvent {
    page: EventPage,
    correlation_id: String,
}

/// Mock event store that stores events in memory.
#[derive(Default)]
pub struct MockEventStore {
    events: RwLock<HashMap<(String, Uuid), Vec<StoredEvent>>>,
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
        let key = (domain.to_string(), root);
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

    async fn get(&self, domain: &str, root: Uuid) -> Result<Vec<EventPage>> {
        if *self.fail_on_get.read().await {
            return Err(StorageError::NotFound {
                domain: domain.to_string(),
                root,
            });
        }
        let key = (domain.to_string(), root);
        let store = self.events.read().await;
        Ok(store
            .get(&key)
            .map(|events| events.iter().map(|e| e.page.clone()).collect())
            .unwrap_or_default())
    }

    async fn get_from(&self, domain: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>> {
        let events = self.get(domain, root).await?;
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
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>> {
        let events = self.get(domain, root).await?;
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
        root: Uuid,
        until: &str,
    ) -> Result<Vec<EventPage>> {
        let events = self.get(domain, root).await?;
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

    async fn list_roots(&self, domain: &str) -> Result<Vec<Uuid>> {
        let store = self.events.read().await;
        Ok(store
            .keys()
            .filter(|(d, _)| d == domain)
            .map(|(_, r)| *r)
            .collect())
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        let store = self.events.read().await;
        let mut domains: Vec<_> = store.keys().map(|(d, _)| d.clone()).collect();
        domains.sort();
        domains.dedup();
        Ok(domains)
    }

    async fn get_next_sequence(&self, domain: &str, root: Uuid) -> Result<u32> {
        if let Some(seq) = *self.next_sequence_override.read().await {
            return Ok(seq);
        }
        let events = self.get(domain, root).await?;
        Ok(events.len() as u32)
    }

    async fn get_by_correlation(&self, correlation_id: &str) -> Result<Vec<EventBook>> {
        if correlation_id.is_empty() {
            return Ok(vec![]);
        }

        let store = self.events.read().await;
        let mut books_map: HashMap<(String, Uuid), Vec<EventPage>> = HashMap::new();

        for ((domain, root), events) in store.iter() {
            for stored in events {
                if stored.correlation_id == correlation_id {
                    books_map
                        .entry((domain.clone(), *root))
                        .or_default()
                        .push(stored.page.clone());
                }
            }
        }

        let mut books = Vec::with_capacity(books_map.len());
        for ((domain, root), pages) in books_map {
            books.push(EventBook {
                cover: Some(Cover {
                    domain,
                    root: Some(ProtoUuid {
                        value: root.as_bytes().to_vec(),
                    }),
                    correlation_id: correlation_id.to_string(),
                }),
                pages,
                snapshot: None,
                snapshot_state: None,
            });
        }

        Ok(books)
    }
}

/// Mock snapshot store that stores snapshots in memory.
#[derive(Default)]
pub struct MockSnapshotStore {
    snapshots: RwLock<HashMap<(String, Uuid), Snapshot>>,
}

impl MockSnapshotStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_stored(&self, domain: &str, root: Uuid) -> Option<Snapshot> {
        let key = (domain.to_string(), root);
        self.snapshots.read().await.get(&key).cloned()
    }

    pub async fn stored_count(&self) -> usize {
        self.snapshots.read().await.len()
    }
}

#[async_trait]
impl SnapshotStore for MockSnapshotStore {
    async fn get(&self, namespace: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let key = (namespace.to_string(), root);
        let store = self.snapshots.read().await;
        Ok(store.get(&key).cloned())
    }

    async fn put(&self, namespace: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let key = (namespace.to_string(), root);
        self.snapshots.write().await.insert(key, snapshot);
        Ok(())
    }

    async fn delete(&self, namespace: &str, root: Uuid) -> Result<()> {
        let key = (namespace.to_string(), root);
        self.snapshots.write().await.remove(&key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_event_store_add_and_get() {
        let store = MockEventStore::new();
        let root = Uuid::new_v4();

        let events = vec![EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Num(0)),
            event: Some(prost_types::Any {
                type_url: "test.Event".to_string(),
                value: vec![],
            }),
            created_at: None,
        }];

        store.add("orders", root, events, "corr-123").await.unwrap();

        let retrieved = store.get("orders", root).await.unwrap();
        assert_eq!(retrieved.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_event_store_get_by_correlation() {
        let store = MockEventStore::new();
        let root1 = Uuid::new_v4();
        let root2 = Uuid::new_v4();

        let event1 = EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Num(0)),
            event: Some(prost_types::Any {
                type_url: "orders.Created".to_string(),
                value: vec![],
            }),
            created_at: None,
        };

        let event2 = EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Num(0)),
            event: Some(prost_types::Any {
                type_url: "payment.Confirmed".to_string(),
                value: vec![],
            }),
            created_at: None,
        };

        // Add events with same correlation_id across different domains
        store
            .add("orders", root1, vec![event1], "tx-abc")
            .await
            .unwrap();
        store
            .add("payment", root2, vec![event2], "tx-abc")
            .await
            .unwrap();

        // Query by correlation_id
        let books = store.get_by_correlation("tx-abc").await.unwrap();
        assert_eq!(books.len(), 2);

        // Query with different correlation_id returns empty
        let empty = store.get_by_correlation("tx-xyz").await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_get_until_timestamp_filters_by_created_at() {
        let store = MockEventStore::new();
        let root = Uuid::new_v4();

        let events = vec![
            EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "test.Event0".to_string(),
                    value: vec![],
                }),
                created_at: Some(prost_types::Timestamp {
                    seconds: 1704067200, // 2024-01-01T00:00:00Z
                    nanos: 0,
                }),
            },
            EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(1)),
                event: Some(prost_types::Any {
                    type_url: "test.Event1".to_string(),
                    value: vec![],
                }),
                created_at: Some(prost_types::Timestamp {
                    seconds: 1704153600, // 2024-01-02T00:00:00Z
                    nanos: 0,
                }),
            },
            EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(2)),
                event: Some(prost_types::Any {
                    type_url: "test.Event2".to_string(),
                    value: vec![],
                }),
                created_at: Some(prost_types::Timestamp {
                    seconds: 1704240000, // 2024-01-03T00:00:00Z
                    nanos: 0,
                }),
            },
        ];
        store.add("orders", root, events, "").await.unwrap();

        // Query as-of Jan 2 — should return events 0 and 1
        let result = store
            .get_until_timestamp("orders", root, "2024-01-02T00:00:00Z")
            .await
            .unwrap();
        assert_eq!(result.len(), 2);

        // Query as-of Jan 1 — should return event 0 only
        let result = store
            .get_until_timestamp("orders", root, "2024-01-01T00:00:00Z")
            .await
            .unwrap();
        assert_eq!(result.len(), 1);

        // Query before any events — should return empty
        let result = store
            .get_until_timestamp("orders", root, "2023-12-31T00:00:00Z")
            .await
            .unwrap();
        assert!(result.is_empty());

        // Query after all events — should return all
        let result = store
            .get_until_timestamp("orders", root, "2024-01-04T00:00:00Z")
            .await
            .unwrap();
        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn test_get_until_timestamp_excludes_events_without_timestamp() {
        let store = MockEventStore::new();
        let root = Uuid::new_v4();

        let events = vec![EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Num(0)),
            event: Some(prost_types::Any {
                type_url: "test.Event".to_string(),
                value: vec![],
            }),
            created_at: None,
        }];
        store.add("orders", root, events, "").await.unwrap();

        let result = store
            .get_until_timestamp("orders", root, "2024-01-02T00:00:00Z")
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_get_until_timestamp_invalid_format() {
        let store = MockEventStore::new();
        let root = Uuid::new_v4();

        let result = store
            .get_until_timestamp("orders", root, "not-a-timestamp")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_snapshot_store() {
        let store = MockSnapshotStore::new();
        let root = Uuid::new_v4();

        let snapshot = Snapshot {
            sequence: 5,
            state: None,
        };

        store.put("orders", root, snapshot.clone()).await.unwrap();

        let retrieved = store.get("orders", root).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().sequence, 5);
    }
}
