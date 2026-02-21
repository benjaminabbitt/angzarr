use super::*;
use crate::test_utils::{make_event_book_with_root, CountingHandler};
use std::sync::atomic::Ordering;
use uuid::Uuid;

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
    let book = Arc::new(make_event_book_with_root("orders", Uuid::new_v4(), vec![]));

    // Should not error even with no receivers
    let result = bus.publish(book).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_channel_subscribe_and_receive() {
    let bus = ChannelEventBus::subscriber_all();

    // Subscribe handler
    let handler = CountingHandler::new();
    let count = handler.count();
    bus.subscribe(Box::new(handler)).await.unwrap();
    bus.start_consuming().await.unwrap();

    // Give consumer time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Publish
    let book = Arc::new(make_event_book_with_root("orders", Uuid::new_v4(), vec![]));
    bus.publish(book).await.unwrap();

    // Give handler time to process
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_channel_domain_filter() {
    let bus = ChannelEventBus::subscriber("orders");

    let handler = CountingHandler::new();
    let count = handler.count();
    bus.subscribe(Box::new(handler)).await.unwrap();
    bus.start_consuming().await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Publish to matching domain
    let book1 = Arc::new(make_event_book_with_root("orders", Uuid::new_v4(), vec![]));
    bus.publish(book1).await.unwrap();

    // Publish to non-matching domain
    let book2 = Arc::new(make_event_book_with_root(
        "inventory",
        Uuid::new_v4(),
        vec![],
    ));
    bus.publish(book2).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Should only count the matching one
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_channel_shared_sender() {
    let publisher = ChannelEventBus::publisher();
    let subscriber = publisher.with_config(ChannelConfig::subscriber_all());

    let handler = CountingHandler::new();
    let count = handler.count();
    subscriber.subscribe(Box::new(handler)).await.unwrap();
    subscriber.start_consuming().await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Publish via publisher, receive via subscriber
    let book = Arc::new(make_event_book_with_root("orders", Uuid::new_v4(), vec![]));
    publisher.publish(book).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    assert_eq!(count.load(Ordering::SeqCst), 1);
}
