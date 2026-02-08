use super::*;
use crate::bus::BusError;
use crate::proto::{Cover, Uuid as ProtoUuid};
use futures::future::BoxFuture;
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

fn make_event_book(domain: &str, root: Uuid) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
    }
}

struct CountingHandler {
    count: Arc<AtomicUsize>,
}

impl EventHandler for CountingHandler {
    fn handle(
        &self,
        _book: Arc<EventBook>,
    ) -> BoxFuture<'static, std::result::Result<(), BusError>> {
        let count = self.count.clone();
        Box::pin(async move {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}

#[test]
fn test_domain_matches_exact() {
    assert!(domain_matches("orders", "orders"));
    assert!(!domain_matches("orders", "inventory"));
}

#[test]
fn test_domain_matches_wildcard() {
    assert!(domain_matches("orders", "#"));
    assert!(domain_matches("anything", "#"));
}

#[test]
fn test_domain_matches_hierarchical() {
    assert!(domain_matches("orders.items", "orders"));
    assert!(domain_matches("orders.items.details", "orders"));
    assert!(!domain_matches("orders", "orders.items"));
    assert!(!domain_matches("ordersextra", "orders")); // No dot separator
}

#[tokio::test]
async fn test_channel_publish_no_receivers() {
    let bus = ChannelEventBus::publisher();
    let book = Arc::new(make_event_book("orders", Uuid::new_v4()));

    // Should not error even with no receivers
    let result = bus.publish(book).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_channel_subscribe_and_receive() {
    let bus = ChannelEventBus::subscriber_all();
    let count = Arc::new(AtomicUsize::new(0));

    // Subscribe handler
    let handler = CountingHandler {
        count: count.clone(),
    };
    bus.subscribe(Box::new(handler)).await.unwrap();
    bus.start_consuming().await.unwrap();

    // Give consumer time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Publish
    let book = Arc::new(make_event_book("orders", Uuid::new_v4()));
    bus.publish(book).await.unwrap();

    // Give handler time to process
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_channel_domain_filter() {
    let bus = ChannelEventBus::subscriber("orders");
    let count = Arc::new(AtomicUsize::new(0));

    let handler = CountingHandler {
        count: count.clone(),
    };
    bus.subscribe(Box::new(handler)).await.unwrap();
    bus.start_consuming().await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Publish to matching domain
    let book1 = Arc::new(make_event_book("orders", Uuid::new_v4()));
    bus.publish(book1).await.unwrap();

    // Publish to non-matching domain
    let book2 = Arc::new(make_event_book("inventory", Uuid::new_v4()));
    bus.publish(book2).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Should only count the matching one
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_channel_shared_sender() {
    let publisher = ChannelEventBus::publisher();
    let subscriber = publisher.with_config(ChannelConfig::subscriber_all());

    let count = Arc::new(AtomicUsize::new(0));
    let handler = CountingHandler {
        count: count.clone(),
    };
    subscriber.subscribe(Box::new(handler)).await.unwrap();
    subscriber.start_consuming().await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Publish via publisher, receive via subscriber
    let book = Arc::new(make_event_book("orders", Uuid::new_v4()));
    publisher.publish(book).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    assert_eq!(count.load(Ordering::SeqCst), 1);
}
