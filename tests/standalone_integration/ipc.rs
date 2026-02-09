//! IPC event bus integration tests.
//!
//! Tests the named-pipe (FIFO) based IPC event bus including broker registration,
//! publisher/subscriber communication, domain filtering, wildcard subscriptions,
//! and fan-out to multiple subscribers.

use std::sync::Arc;

use uuid::Uuid;

use crate::common::*;

#[test]
fn test_broker_creates_subscriber_pipes() {
    let base_path = temp_dir();
    let config = IpcBrokerConfig {
        base_path: base_path.clone(),
    };

    let mut broker = IpcBroker::new(config);

    // Register subscriber
    let info = broker
        .register_subscriber("test-projector", vec!["orders".to_string()])
        .expect("Failed to register subscriber");

    assert_eq!(info.name, "test-projector");
    assert_eq!(info.domains, vec!["orders".to_string()]);
    assert!(info.pipe_path.exists(), "Pipe should be created");

    // Verify pipe is a FIFO
    let metadata = std::fs::metadata(&info.pipe_path).expect("Failed to get metadata");
    assert!(
        metadata.file_type().is_fifo(),
        "Should be a named pipe (FIFO)"
    );

    cleanup_dir(&base_path);
}

#[test]
fn test_broker_returns_all_subscribers() {
    let base_path = temp_dir();
    let config = IpcBrokerConfig {
        base_path: base_path.clone(),
    };

    let mut broker = IpcBroker::new(config);

    broker
        .register_subscriber("projector-a", vec!["orders".to_string()])
        .unwrap();
    broker
        .register_subscriber("projector-b", vec!["products".to_string()])
        .unwrap();
    broker
        .register_subscriber("saga-fulfillment", vec!["orders".to_string()])
        .unwrap();

    let subscribers = broker.get_subscribers();
    assert_eq!(subscribers.len(), 3);

    let names: Vec<_> = subscribers.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"projector-a"));
    assert!(names.contains(&"projector-b"));
    assert!(names.contains(&"saga-fulfillment"));

    cleanup_dir(&base_path);
}

#[tokio::test]
async fn test_publisher_writes_to_subscriber_pipes() {
    let base_path = temp_dir();
    let broker_config = IpcBrokerConfig {
        base_path: base_path.clone(),
    };

    let mut broker = IpcBroker::new(broker_config);

    // Register subscriber
    let sub_info = broker
        .register_subscriber("test-sub", vec!["orders".to_string()])
        .unwrap();

    // Create subscriber bus and handler
    let subscriber =
        IpcEventBus::subscriber(base_path.clone(), "test-sub", vec!["orders".to_string()]);
    let handler_state = RecordingHandlerState::new();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();

    // Start consumer in background (blocks until writer connects)
    let sub_clone = Arc::new(subscriber);
    let sub_for_task = sub_clone.clone();
    let _consumer_task = tokio::spawn(async move {
        sub_for_task.start_consuming().await.unwrap();
    });

    // Give consumer time to open pipe
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create publisher with subscriber info
    let publisher_config = IpcConfig::publisher_with_subscribers(base_path.clone(), vec![sub_info]);
    let publisher = IpcEventBus::new(publisher_config);

    // Publish event
    let event = create_test_event_book("orders", Uuid::new_v4(), 0);
    publisher.publish(Arc::new(event)).await.unwrap();

    // Wait for event to be received
    tokio::time::sleep(Duration::from_millis(200)).await;

    let count = handler_state.received_count().await;
    assert_eq!(count, 1, "Should receive one event");

    sub_clone.stop().await;
    cleanup_dir(&base_path);
}

#[tokio::test]
async fn test_domain_filtering() {
    let base_path = temp_dir();
    let broker_config = IpcBrokerConfig {
        base_path: base_path.clone(),
    };

    let mut broker = IpcBroker::new(broker_config);

    // Register subscriber for "orders" domain only
    let sub_info = broker
        .register_subscriber("orders-only", vec!["orders".to_string()])
        .unwrap();

    // Create subscriber
    let subscriber =
        IpcEventBus::subscriber(base_path.clone(), "orders-only", vec!["orders".to_string()]);
    let handler_state = RecordingHandlerState::new();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();

    let sub_clone = Arc::new(subscriber);
    let sub_for_task = sub_clone.clone();
    let _consumer_task = tokio::spawn(async move {
        sub_for_task.start_consuming().await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create publisher
    let publisher_config = IpcConfig::publisher_with_subscribers(base_path.clone(), vec![sub_info]);
    let publisher = IpcEventBus::new(publisher_config);

    // Publish event to "orders" domain (should be received)
    let orders_event = create_test_event_book("orders", Uuid::new_v4(), 0);
    publisher.publish(Arc::new(orders_event)).await.unwrap();

    // Publish event to "products" domain (should be filtered out by publisher)
    let products_event = create_test_event_book("products", Uuid::new_v4(), 0);
    publisher.publish(Arc::new(products_event)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    let count = handler_state.received_count().await;
    assert_eq!(count, 1, "Should receive only orders event");

    let events = handler_state.get_events().await;
    assert_eq!(
        events[0].cover.as_ref().unwrap().domain,
        "orders",
        "Should be orders domain"
    );

    sub_clone.stop().await;
    cleanup_dir(&base_path);
}

#[tokio::test]
async fn test_wildcard_subscriber_receives_all() {
    let base_path = temp_dir();
    let broker_config = IpcBrokerConfig {
        base_path: base_path.clone(),
    };

    let mut broker = IpcBroker::new(broker_config);

    // Register subscriber with wildcard (empty domains = all)
    let sub_info = broker
        .register_subscriber("all-events", vec!["#".to_string()])
        .unwrap();

    // Create subscriber with wildcard
    let subscriber =
        IpcEventBus::subscriber(base_path.clone(), "all-events", vec!["#".to_string()]);
    let handler_state = RecordingHandlerState::new();
    subscriber
        .subscribe(Box::new(RecordingHandler::new(handler_state.clone())))
        .await
        .unwrap();

    let sub_clone = Arc::new(subscriber);
    let sub_for_task = sub_clone.clone();
    let _consumer_task = tokio::spawn(async move {
        sub_for_task.start_consuming().await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create publisher
    let publisher_config = IpcConfig::publisher_with_subscribers(base_path.clone(), vec![sub_info]);
    let publisher = IpcEventBus::new(publisher_config);

    // Publish to multiple domains
    for domain in ["orders", "products", "customers"] {
        let event = create_test_event_book(domain, Uuid::new_v4(), 0);
        publisher.publish(Arc::new(event)).await.unwrap();
    }

    tokio::time::sleep(Duration::from_millis(300)).await;

    let count = handler_state.received_count().await;
    assert_eq!(count, 3, "Should receive all three events");

    sub_clone.stop().await;
    cleanup_dir(&base_path);
}

#[tokio::test]
async fn test_multiple_subscribers_receive_same_event() {
    let base_path = temp_dir();
    let broker_config = IpcBrokerConfig {
        base_path: base_path.clone(),
    };

    let mut broker = IpcBroker::new(broker_config);

    // Register two subscribers for same domain
    let sub_info_a = broker
        .register_subscriber("sub-a", vec!["orders".to_string()])
        .unwrap();
    let sub_info_b = broker
        .register_subscriber("sub-b", vec!["orders".to_string()])
        .unwrap();

    // Create subscribers
    let subscriber_a =
        IpcEventBus::subscriber(base_path.clone(), "sub-a", vec!["orders".to_string()]);
    let handler_state_a = RecordingHandlerState::new();
    subscriber_a
        .subscribe(Box::new(RecordingHandler::new(handler_state_a.clone())))
        .await
        .unwrap();

    let subscriber_b =
        IpcEventBus::subscriber(base_path.clone(), "sub-b", vec!["orders".to_string()]);
    let handler_state_b = RecordingHandlerState::new();
    subscriber_b
        .subscribe(Box::new(RecordingHandler::new(handler_state_b.clone())))
        .await
        .unwrap();

    // Start consumers
    let sub_a_clone = Arc::new(subscriber_a);
    let sub_a_for_task = sub_a_clone.clone();
    let _task_a = tokio::spawn(async move {
        sub_a_for_task.start_consuming().await.unwrap();
    });

    let sub_b_clone = Arc::new(subscriber_b);
    let sub_b_for_task = sub_b_clone.clone();
    let _task_b = tokio::spawn(async move {
        sub_b_for_task.start_consuming().await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create publisher with both subscribers
    let publisher_config =
        IpcConfig::publisher_with_subscribers(base_path.clone(), vec![sub_info_a, sub_info_b]);
    let publisher = IpcEventBus::new(publisher_config);

    // Publish event
    let event = create_test_event_book("orders", Uuid::new_v4(), 0);
    publisher.publish(Arc::new(event)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    let count_a = handler_state_a.received_count().await;
    let count_b = handler_state_b.received_count().await;

    assert_eq!(count_a, 1, "Subscriber A should receive event");
    assert_eq!(count_b, 1, "Subscriber B should receive event");

    sub_a_clone.stop().await;
    sub_b_clone.stop().await;
    cleanup_dir(&base_path);
}
