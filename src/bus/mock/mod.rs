//! Mock event bus implementation for testing.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::{BusError, EventBus, EventHandler, PublishResult, Result};
use crate::proto::EventBook;

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
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        if *self.fail_on_publish.read().await {
            return Err(BusError::Connection("Mock publish failure".to_string()));
        }
        self.published.write().await.push((*book).clone());
        Ok(PublishResult::default())
    }

    async fn subscribe(&self, _handler: Box<dyn EventHandler>) -> Result<()> {
        Err(BusError::SubscribeNotSupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Cover, Uuid as ProtoUuid};
    use uuid::Uuid;

    fn make_event_book(domain: &str, root: Uuid, event_count: usize) -> EventBook {
        use crate::proto::EventPage;

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
                })
                .collect(),
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
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
            ) -> futures::future::BoxFuture<'static, std::result::Result<(), BusError>> {
                Box::pin(async { Ok(()) })
            }
        }

        let result = bus.subscribe(Box::new(DummyHandler)).await;
        assert!(matches!(result, Err(BusError::SubscribeNotSupported)));
    }
}
