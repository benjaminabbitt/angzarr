//! Tests for the in-process channel-based event bus.
//!
//! The channel bus is the primary event transport for standalone mode. It uses
//! Tokio broadcast channels for pub/sub within a single process. Key behaviors:
//!
//! - Domain filtering: Subscribers only receive events from matching domains
//! - Wildcard subscription: "#" matches all domains
//! - Hierarchical domains: "orders.items" matches subscriber "orders"
//! - Graceful degradation: Publishing with no subscribers succeeds silently
//!
//! These tests validate the local pub/sub semantics that higher-level components
//! depend on for event routing.

use super::*;
use crate::test_utils::{make_event_book_with_root, CountingHandler};
use std::sync::atomic::Ordering;
use uuid::Uuid;

// ============================================================================
// Domain Matching Tests
// ============================================================================

/// Exact domain match requires identical strings.
#[test]
fn test_domain_matches_exact() {
    assert!(domain_matches("orders", "orders"));
    assert!(!domain_matches("orders", "inventory"));
}

/// Wildcard "#" matches any domain — used for projectors that consume all events.
#[test]
fn test_domain_matches_wildcard() {
    assert!(domain_matches("orders", "#"));
    assert!(domain_matches("anything", "#"));
}

/// Hierarchical domains match parent subscriptions.
///
/// Supports domain namespacing: "orders.items" events route to "orders" subscribers.
/// The dot separator is significant — "ordersextra" does not match "orders".
#[test]
fn test_domain_matches_hierarchical() {
    assert!(domain_matches("orders.items", "orders"));
    assert!(domain_matches("orders.items.details", "orders"));
    assert!(!domain_matches("orders", "orders.items"));
    assert!(!domain_matches("ordersextra", "orders")); // No dot separator
}

// ============================================================================
// Publish/Subscribe Tests
// ============================================================================

/// Publishing without subscribers succeeds without error.
///
/// Events may be published before subscribers connect. The bus should not
/// fail — events are simply dropped. Subscribers are responsible for
/// catching up via event replay if needed.
#[tokio::test]
async fn test_channel_publish_no_receivers() {
    let bus = ChannelEventBus::publisher();
    let book = Arc::new(make_event_book_with_root("orders", Uuid::new_v4(), vec![]));

    // Should not error even with no receivers
    let result = bus.publish(book).await;
    assert!(result.is_ok());
}

/// Subscribed handler receives published events.
///
/// Basic pub/sub contract: events published after subscription are delivered
/// to the handler. This is the fundamental behavior all consumers rely on.
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

/// Domain filter prevents delivery of non-matching events.
///
/// Subscribers declare which domain they care about. Events from other domains
/// are silently dropped. This enables efficient routing without handler-side
/// filtering.
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

/// Publisher and subscriber share the same underlying channel.
///
/// Multiple bus instances can share a channel via with_config(). This enables
/// the common pattern: aggregates publish via one bus, sagas/projectors
/// subscribe via another, all routing through the same channel.
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
