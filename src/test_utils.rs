//! Test utilities and mock implementations.
//!
//! This module provides mock implementations of core traits for testing
//! without requiring actual database or gRPC connections.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::interfaces::business_client::{
    BusinessError, BusinessLogicClient, Result as BusinessResult,
};
use crate::interfaces::event_bus::{
    BusError, EventBus, EventHandler, PublishResult, Result as BusResult,
};
use crate::interfaces::event_store::{EventStore, Result as StorageResult, StorageError};
use crate::interfaces::snapshot_store::SnapshotStore;
use crate::proto::{
    business_response, BusinessResponse, ContextualCommand, Cover, EventBook, EventPage, Snapshot,
    Uuid as ProtoUuid,
};

/// Mock event store that stores events in memory.
#[derive(Default)]
pub struct MockEventStore {
    events: RwLock<HashMap<(String, Uuid), Vec<EventPage>>>,
    fail_on_add: RwLock<bool>,
    fail_on_get: RwLock<bool>,
    /// Override for get_next_sequence. When set, returns this value instead of events.len().
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

    /// Set the next sequence value to return from get_next_sequence.
    /// When set, overrides the computed value (events.len()).
    pub async fn set_next_sequence(&self, seq: u32) {
        *self.next_sequence_override.write().await = Some(seq);
    }

    /// Clear the next sequence override, reverting to computed value.
    pub async fn clear_next_sequence_override(&self) {
        *self.next_sequence_override.write().await = None;
    }
}

#[async_trait]
impl EventStore for MockEventStore {
    async fn add(&self, domain: &str, root: Uuid, events: Vec<EventPage>) -> StorageResult<()> {
        if *self.fail_on_add.read().await {
            return Err(StorageError::Database(sqlx::Error::RowNotFound));
        }
        let key = (domain.to_string(), root);
        let mut store = self.events.write().await;
        store.entry(key).or_default().extend(events);
        Ok(())
    }

    async fn get(&self, domain: &str, root: Uuid) -> StorageResult<Vec<EventPage>> {
        if *self.fail_on_get.read().await {
            return Err(StorageError::Database(sqlx::Error::RowNotFound));
        }
        let key = (domain.to_string(), root);
        let store = self.events.read().await;
        Ok(store.get(&key).cloned().unwrap_or_default())
    }

    async fn get_from(&self, domain: &str, root: Uuid, from: u32) -> StorageResult<Vec<EventPage>> {
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
    ) -> StorageResult<Vec<EventPage>> {
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

    async fn list_roots(&self, domain: &str) -> StorageResult<Vec<Uuid>> {
        let store = self.events.read().await;
        Ok(store
            .keys()
            .filter(|(d, _)| d == domain)
            .map(|(_, r)| *r)
            .collect())
    }

    async fn list_domains(&self) -> StorageResult<Vec<String>> {
        let store = self.events.read().await;
        let mut domains: Vec<_> = store.keys().map(|(d, _)| d.clone()).collect();
        domains.sort();
        domains.dedup();
        Ok(domains)
    }

    async fn get_next_sequence(&self, domain: &str, root: Uuid) -> StorageResult<u32> {
        // Return override if set, otherwise compute from events
        if let Some(seq) = *self.next_sequence_override.read().await {
            return Ok(seq);
        }
        let events = self.get(domain, root).await?;
        Ok(events.len() as u32)
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
    async fn get(&self, domain: &str, root: Uuid) -> StorageResult<Option<Snapshot>> {
        let key = (domain.to_string(), root);
        let store = self.snapshots.read().await;
        Ok(store.get(&key).cloned())
    }

    async fn put(&self, domain: &str, root: Uuid, snapshot: Snapshot) -> StorageResult<()> {
        let key = (domain.to_string(), root);
        self.snapshots.write().await.insert(key, snapshot);
        Ok(())
    }

    async fn delete(&self, domain: &str, root: Uuid) -> StorageResult<()> {
        let key = (domain.to_string(), root);
        self.snapshots.write().await.remove(&key);
        Ok(())
    }
}

/// Mock business logic client for testing.
pub struct MockBusinessLogic {
    domains: Vec<String>,
    fail_on_handle: RwLock<bool>,
    reject_command: RwLock<bool>,
    return_snapshot: RwLock<bool>,
}

impl MockBusinessLogic {
    pub fn new(domains: Vec<String>) -> Self {
        Self {
            domains,
            fail_on_handle: RwLock::new(false),
            reject_command: RwLock::new(false),
            return_snapshot: RwLock::new(false),
        }
    }

    pub async fn set_fail_on_handle(&self, fail: bool) {
        *self.fail_on_handle.write().await = fail;
    }

    pub async fn set_reject_command(&self, reject: bool) {
        *self.reject_command.write().await = reject;
    }

    pub async fn set_return_snapshot(&self, return_snapshot: bool) {
        *self.return_snapshot.write().await = return_snapshot;
    }
}

#[async_trait]
impl BusinessLogicClient for MockBusinessLogic {
    async fn handle(
        &self,
        domain: &str,
        cmd: ContextualCommand,
    ) -> BusinessResult<BusinessResponse> {
        if *self.fail_on_handle.read().await {
            return Err(BusinessError::Connection {
                domain: domain.to_string(),
                message: "Mock connection failure".to_string(),
            });
        }

        if *self.reject_command.read().await {
            return Err(BusinessError::Rejected("Mock rejection".to_string()));
        }

        if !self.has_domain(domain) {
            return Err(BusinessError::DomainNotFound(domain.to_string()));
        }

        // Generate a simple event from the command
        let cover = cmd.command.and_then(|c| c.cover);
        let prior_seq = cmd
            .events
            .as_ref()
            .map(|e| e.pages.len() as u32)
            .unwrap_or(0);

        // Optionally include snapshot state (framework computes sequence)
        let snapshot_state = if *self.return_snapshot.read().await {
            Some(prost_types::Any {
                type_url: "test.MockState".to_string(),
                value: vec![1, 2, 3],
            })
        } else {
            None
        };

        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(EventBook {
                cover,
                pages: vec![EventPage {
                    sequence: Some(crate::proto::event_page::Sequence::Num(prior_seq)),
                    event: Some(prost_types::Any {
                        type_url: "test.MockEvent".to_string(),
                        value: vec![],
                    }),
                    created_at: None,
                    synchronous: false,
                }],
                snapshot: None, // Framework-populated on load, not set by business logic
                correlation_id: String::new(),
                snapshot_state,
            })),
        })
    }

    fn has_domain(&self, domain: &str) -> bool {
        self.domains.contains(&domain.to_string())
    }

    fn domains(&self) -> Vec<String> {
        self.domains.clone()
    }
}

/// Mock event bus for testing.
#[derive(Default)]
pub struct MockEventBus {
    published: RwLock<Vec<EventBook>>,
    fail_on_publish: RwLock<bool>,
}

impl MockEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_fail_on_publish(&self, fail: bool) {
        *self.fail_on_publish.write().await = fail;
    }

    pub async fn published_count(&self) -> usize {
        self.published.read().await.len()
    }

    pub async fn take_published(&self) -> Vec<EventBook> {
        std::mem::take(&mut *self.published.write().await)
    }
}

#[async_trait]
impl EventBus for MockEventBus {
    async fn publish(&self, book: Arc<EventBook>) -> BusResult<PublishResult> {
        if *self.fail_on_publish.read().await {
            return Err(BusError::Connection("Mock publish failure".to_string()));
        }
        self.published.write().await.push((*book).clone());
        Ok(PublishResult::default())
    }

    async fn subscribe(&self, _handler: Box<dyn EventHandler>) -> BusResult<()> {
        Err(BusError::SubscribeNotSupported)
    }
}

/// Helper to create a valid EventBook for testing.
pub fn make_event_book(domain: &str, root: Uuid, event_count: usize) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
        }),
        pages: (0..event_count)
            .map(|i| EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(i as u32)),
                event: Some(prost_types::Any {
                    type_url: format!("test.Event{}", i),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            })
            .collect(),
        snapshot: None,
        correlation_id: String::new(),
        snapshot_state: None,
    }
}

/// Helper to create a valid ProtoUuid from a Uuid.
pub fn make_proto_uuid(uuid: Uuid) -> ProtoUuid {
    ProtoUuid {
        value: uuid.as_bytes().to_vec(),
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
            synchronous: false,
        }];

        store.add("orders", root, events).await.unwrap();

        let retrieved = store.get("orders", root).await.unwrap();
        assert_eq!(retrieved.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_event_store_fail_on_add() {
        let store = MockEventStore::new();
        store.set_fail_on_add(true).await;

        let result = store.add("orders", Uuid::new_v4(), vec![]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_event_store_fail_on_get() {
        let store = MockEventStore::new();
        store.set_fail_on_get(true).await;

        let result = store.get("orders", Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_event_store_list_roots() {
        let store = MockEventStore::new();
        let root1 = Uuid::new_v4();
        let root2 = Uuid::new_v4();

        store.add("orders", root1, vec![]).await.unwrap();
        store.add("orders", root2, vec![]).await.unwrap();

        let roots = store.list_roots("orders").await.unwrap();
        assert_eq!(roots.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_event_store_list_domains() {
        let store = MockEventStore::new();

        store.add("orders", Uuid::new_v4(), vec![]).await.unwrap();
        store
            .add("inventory", Uuid::new_v4(), vec![])
            .await
            .unwrap();

        let domains = store.list_domains().await.unwrap();
        assert_eq!(domains.len(), 2);
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

        store.delete("orders", root).await.unwrap();
        let deleted = store.get("orders", root).await.unwrap();
        assert!(deleted.is_none());
    }

    #[tokio::test]
    async fn test_mock_business_logic_handle() {
        let logic = MockBusinessLogic::new(vec!["orders".to_string()]);

        let cmd = ContextualCommand {
            events: None,
            command: None,
        };

        let result = logic.handle("orders", cmd).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_business_logic_domain_not_found() {
        let logic = MockBusinessLogic::new(vec!["orders".to_string()]);

        let cmd = ContextualCommand {
            events: None,
            command: None,
        };

        let result = logic.handle("unknown", cmd).await;
        assert!(matches!(result, Err(BusinessError::DomainNotFound(_))));
    }

    #[tokio::test]
    async fn test_mock_business_logic_fail_on_handle() {
        let logic = MockBusinessLogic::new(vec!["orders".to_string()]);
        logic.set_fail_on_handle(true).await;

        let cmd = ContextualCommand {
            events: None,
            command: None,
        };

        let result = logic.handle("orders", cmd).await;
        assert!(matches!(result, Err(BusinessError::Connection { .. })));
    }

    #[tokio::test]
    async fn test_mock_business_logic_reject_command() {
        let logic = MockBusinessLogic::new(vec!["orders".to_string()]);
        logic.set_reject_command(true).await;

        let cmd = ContextualCommand {
            events: None,
            command: None,
        };

        let result = logic.handle("orders", cmd).await;
        assert!(matches!(result, Err(BusinessError::Rejected(_))));
    }

    #[tokio::test]
    async fn test_mock_event_bus_publish() {
        let bus = MockEventBus::new();
        let book = Arc::new(make_event_book("orders", Uuid::new_v4(), 1));

        bus.publish(book).await.unwrap();

        assert_eq!(bus.published_count().await, 1);
    }

    #[tokio::test]
    async fn test_mock_event_bus_fail_on_publish() {
        let bus = MockEventBus::new();
        bus.set_fail_on_publish(true).await;

        let book = Arc::new(make_event_book("orders", Uuid::new_v4(), 1));
        let result = bus.publish(book).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_event_bus_subscribe_not_supported() {
        let bus = MockEventBus::new();

        struct DummyHandler;
        impl EventHandler for DummyHandler {
            fn handle(
                &self,
                _book: Arc<EventBook>,
            ) -> futures::future::BoxFuture<'static, Result<(), BusError>> {
                Box::pin(async { Ok(()) })
            }
        }

        let result = bus.subscribe(Box::new(DummyHandler)).await;
        assert!(matches!(result, Err(BusError::SubscribeNotSupported)));
    }

    #[tokio::test]
    async fn test_mock_event_store_get_from() {
        let store = MockEventStore::new();
        let root = Uuid::new_v4();

        let events: Vec<EventPage> = (0..5)
            .map(|i| EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(i)),
                event: Some(prost_types::Any {
                    type_url: format!("test.Event{}", i),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            })
            .collect();

        store.add("orders", root, events).await.unwrap();

        let from_2 = store.get_from("orders", root, 2).await.unwrap();
        assert_eq!(from_2.len(), 3); // Events 2, 3, 4
    }

    #[tokio::test]
    async fn test_mock_event_store_get_from_to() {
        let store = MockEventStore::new();
        let root = Uuid::new_v4();

        let events: Vec<EventPage> = (0..5)
            .map(|i| EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(i)),
                event: Some(prost_types::Any {
                    type_url: format!("test.Event{}", i),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            })
            .collect();

        store.add("orders", root, events).await.unwrap();

        let range = store.get_from_to("orders", root, 1, 4).await.unwrap();
        assert_eq!(range.len(), 3); // Events 1, 2, 3
    }

    #[tokio::test]
    async fn test_mock_event_store_get_next_sequence() {
        let store = MockEventStore::new();
        let root = Uuid::new_v4();

        let events: Vec<EventPage> = (0..3)
            .map(|i| EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(i)),
                event: None,
                created_at: None,
                synchronous: false,
            })
            .collect();

        store.add("orders", root, events).await.unwrap();

        let next = store.get_next_sequence("orders", root).await.unwrap();
        assert_eq!(next, 3);
    }
}
