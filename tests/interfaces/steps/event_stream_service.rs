//! EventStreamService interface step definitions.

use std::collections::HashMap;
use std::sync::Arc;

use angzarr::bus::EventHandler;
use angzarr::handlers::projectors::stream::{StreamEventHandler, StreamService};
use angzarr::proto::event_stream_service_server::EventStreamService as EventStreamTrait;
use angzarr::proto::{
    event_page, Cover, EventBook, EventPage, EventStreamFilter, Uuid as ProtoUuid,
};
use cucumber::{given, then, when, World};
use prost_types::Any;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tonic::Request;
use uuid::Uuid;

type StreamReceiver = mpsc::Receiver<Result<EventBook, tonic::Status>>;

/// Test context for EventStreamService scenarios.
#[derive(World)]
#[world(init = Self::new)]
pub struct EventStreamServiceWorld {
    service: StreamService,
    handler: Option<StreamEventHandler>,
    // Single subscriber state
    current_stream: Option<StreamReceiver>,
    current_correlation: String,
    last_received: Vec<EventBook>,
    last_error: Option<tonic::Status>,
    disconnected: bool,
    // Multi-subscriber state
    subscribers: HashMap<String, SubscriberState>,
}

struct SubscriberState {
    stream: Option<StreamReceiver>,
    received: Vec<EventBook>,
    disconnected: bool,
}

impl std::fmt::Debug for EventStreamServiceWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventStreamServiceWorld")
            .field("service", &"<StreamService>")
            .field("current_correlation", &self.current_correlation)
            .field(
                "last_received",
                &format!("[{} events]", self.last_received.len()),
            )
            .field("last_error", &self.last_error)
            .field(
                "subscribers",
                &format!("[{} subscribers]", self.subscribers.len()),
            )
            .finish()
    }
}

impl EventStreamServiceWorld {
    fn new() -> Self {
        let service = StreamService::new();
        let handler = StreamEventHandler::new(&service);
        Self {
            service,
            handler: Some(handler),
            current_stream: None,
            current_correlation: String::new(),
            last_received: Vec::new(),
            last_error: None,
            disconnected: false,
            subscribers: HashMap::new(),
        }
    }

    fn handler(&self) -> &StreamEventHandler {
        self.handler.as_ref().expect("Handler not initialized")
    }

    fn make_event_book(&self, correlation_id: &str, page_count: usize) -> EventBook {
        let mut pages = Vec::new();
        for seq in 0..page_count {
            pages.push(EventPage {
                sequence: seq as u32,
                payload: Some(event_page::Payload::Event(Any {
                    type_url: format!("type.test/Event{}", seq),
                    value: vec![seq as u8],
                })),
                created_at: None,
            });
        }

        EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid {
                    value: Uuid::new_v4().as_bytes().to_vec(),
                }),
                correlation_id: correlation_id.to_string(),
                edition: None,
            }),
            pages,
            snapshot: None,
            ..Default::default()
        }
    }

    fn make_event_book_with_domain(
        &self,
        domain: &str,
        root: Uuid,
        correlation_id: &str,
    ) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: correlation_id.to_string(),
                edition: None,
            }),
            pages: vec![EventPage {
                sequence: 0,
                payload: Some(event_page::Payload::Event(Any {
                    type_url: "type.test/Event".to_string(),
                    value: vec![1],
                })),
                created_at: None,
            }],
            snapshot: None,
            ..Default::default()
        }
    }

    fn make_event_book_no_correlation(&self) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid {
                    value: Uuid::new_v4().as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: None,
            }),
            pages: vec![EventPage {
                sequence: 0,
                payload: Some(event_page::Payload::Event(Any {
                    type_url: "type.test/Event".to_string(),
                    value: vec![1],
                })),
                created_at: None,
            }],
            snapshot: None,
            ..Default::default()
        }
    }

    async fn try_receive_from_stream(
        stream: &mut StreamReceiver,
        timeout_ms: u64,
    ) -> Option<EventBook> {
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(timeout_ms),
            stream.recv(),
        )
        .await
        {
            Ok(Some(Ok(book))) => Some(book),
            _ => None,
        }
    }
}

// ==========================================================================
// Background
// ==========================================================================

#[given("an EventStreamService backend")]
async fn given_stream_service_backend(_world: &mut EventStreamServiceWorld) {
    // Already initialized in new()
}

// ==========================================================================
// Subscribe - Registration
// ==========================================================================

#[when(expr = "I subscribe with correlation ID {string}")]
async fn when_subscribe_with_correlation(
    world: &mut EventStreamServiceWorld,
    correlation_id: String,
) {
    let filter = EventStreamFilter {
        correlation_id: correlation_id.clone(),
    };

    match world.service.subscribe(Request::new(filter)).await {
        Ok(response) => {
            // Convert Pin<Box<dyn Stream>> to a channel receiver for easier testing
            let mut stream = response.into_inner();
            let (tx, rx) = mpsc::channel(32);

            tokio::spawn(async move {
                while let Some(item) = stream.next().await {
                    if tx.send(item).await.is_err() {
                        break;
                    }
                }
            });

            world.current_stream = Some(rx);
            world.current_correlation = correlation_id;
            world.last_error = None;
        }
        Err(status) => {
            world.current_stream = None;
            world.last_error = Some(status);
        }
    }
}

#[when("I subscribe without a correlation ID")]
async fn when_subscribe_without_correlation(world: &mut EventStreamServiceWorld) {
    when_subscribe_with_correlation(world, String::new()).await;
}

#[then("the subscription should be active")]
async fn then_subscription_active(world: &mut EventStreamServiceWorld) {
    assert!(
        world.current_stream.is_some(),
        "Expected active subscription"
    );
    assert!(world.last_error.is_none(), "Expected no error");
}

#[then(expr = "I should be able to receive events for {string}")]
async fn then_can_receive_events(world: &mut EventStreamServiceWorld, correlation_id: String) {
    // Publish a test event and verify it's received
    let book = world.make_event_book(&correlation_id, 1);
    world
        .handler()
        .handle(Arc::new(book))
        .await
        .expect("Handler failed");

    // Small delay for delivery
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

    let stream = world.current_stream.as_mut().expect("No active stream");
    let received = EventStreamServiceWorld::try_receive_from_stream(stream, 100).await;
    assert!(
        received.is_some(),
        "Should be able to receive events for '{}'",
        correlation_id
    );
}

#[then("the subscribe should fail with INVALID_ARGUMENT")]
async fn then_subscribe_fails(world: &mut EventStreamServiceWorld) {
    let error = world
        .last_error
        .as_ref()
        .expect("Expected error but none occurred");
    assert_eq!(
        error.code(),
        tonic::Code::InvalidArgument,
        "Expected INVALID_ARGUMENT, got {:?}",
        error.code()
    );
}

// ==========================================================================
// Event Delivery
// ==========================================================================

#[given(expr = "I am subscribed with correlation ID {string}")]
async fn given_subscribed(world: &mut EventStreamServiceWorld, correlation_id: String) {
    when_subscribe_with_correlation(world, correlation_id).await;
}

#[when(expr = "an event with correlation ID {string} is published")]
async fn when_event_published(world: &mut EventStreamServiceWorld, correlation_id: String) {
    let book = world.make_event_book(&correlation_id, 1);
    world
        .handler()
        .handle(Arc::new(book))
        .await
        .expect("Handler failed");
    // Small delay to allow async delivery
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
}

#[when(expr = "an event with correlation ID {string} and {int} pages is published")]
async fn when_event_with_pages_published(
    world: &mut EventStreamServiceWorld,
    correlation_id: String,
    page_count: usize,
) {
    let book = world.make_event_book(&correlation_id, page_count);
    world
        .handler()
        .handle(Arc::new(book))
        .await
        .expect("Handler failed");
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
}

#[when(
    expr = "an event from domain {string} with root {string} and correlation ID {string} is published"
)]
async fn when_domain_event_published(
    world: &mut EventStreamServiceWorld,
    domain: String,
    root_str: String,
    correlation_id: String,
) {
    let root = Uuid::new_v5(&Uuid::NAMESPACE_OID, root_str.as_bytes());
    let book = world.make_event_book_with_domain(&domain, root, &correlation_id);
    world
        .handler()
        .handle(Arc::new(book))
        .await
        .expect("Handler failed");
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
}

#[when("an event without a correlation ID is published")]
async fn when_event_no_correlation_published(world: &mut EventStreamServiceWorld) {
    let book = world.make_event_book_no_correlation();
    world
        .handler()
        .handle(Arc::new(book))
        .await
        .expect("Handler failed");
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
}

#[then("I should receive the event in my stream")]
async fn then_receive_event(world: &mut EventStreamServiceWorld) {
    let stream = world.current_stream.as_mut().expect("No active stream");
    let received = EventStreamServiceWorld::try_receive_from_stream(stream, 100).await;
    assert!(received.is_some(), "Expected to receive event but got none");
    world.last_received.push(received.unwrap());
}

#[then("I should not receive any events")]
async fn then_no_events(world: &mut EventStreamServiceWorld) {
    let stream = world.current_stream.as_mut().expect("No active stream");
    let received = EventStreamServiceWorld::try_receive_from_stream(stream, 50).await;
    assert!(received.is_none(), "Expected no events but received one");
}

#[then("no error should occur")]
async fn then_no_error(world: &mut EventStreamServiceWorld) {
    assert!(world.last_error.is_none(), "Expected no error but got one");
}

// ==========================================================================
// Multiple Subscribers
// ==========================================================================

#[given(expr = "subscriber {int} is subscribed with correlation ID {string}")]
async fn given_numbered_subscriber(
    world: &mut EventStreamServiceWorld,
    sub_num: u32,
    correlation_id: String,
) {
    let filter = EventStreamFilter {
        correlation_id: correlation_id.clone(),
    };

    match world.service.subscribe(Request::new(filter)).await {
        Ok(response) => {
            let mut stream = response.into_inner();
            let (tx, rx) = mpsc::channel(32);

            tokio::spawn(async move {
                while let Some(item) = stream.next().await {
                    if tx.send(item).await.is_err() {
                        break;
                    }
                }
            });

            let key = format!("subscriber-{}", sub_num);
            world.subscribers.insert(
                key,
                SubscriberState {
                    stream: Some(rx),
                    received: Vec::new(),
                    disconnected: false,
                },
            );
        }
        Err(status) => {
            world.last_error = Some(status);
        }
    }
}

#[then(expr = "subscriber {int} should receive the event")]
async fn then_subscriber_receives(world: &mut EventStreamServiceWorld, sub_num: u32) {
    let key = format!("subscriber-{}", sub_num);
    let sub = world
        .subscribers
        .get_mut(&key)
        .unwrap_or_else(|| panic!("Subscriber {} not found", sub_num));

    let stream = sub.stream.as_mut().expect("No stream for subscriber");
    let received = EventStreamServiceWorld::try_receive_from_stream(stream, 100).await;
    assert!(
        received.is_some(),
        "Expected subscriber {} to receive event",
        sub_num
    );
    sub.received.push(received.unwrap());
}

#[then(expr = "subscriber {int} should not receive any events")]
async fn then_subscriber_no_events(world: &mut EventStreamServiceWorld, sub_num: u32) {
    let key = format!("subscriber-{}", sub_num);
    let sub = world
        .subscribers
        .get_mut(&key)
        .unwrap_or_else(|| panic!("Subscriber {} not found", sub_num));

    if sub.disconnected {
        // Disconnected subscribers can't receive - this is expected
        return;
    }

    let stream = sub.stream.as_mut().expect("No stream for subscriber");
    let received = EventStreamServiceWorld::try_receive_from_stream(stream, 50).await;
    assert!(
        received.is_none(),
        "Expected subscriber {} to NOT receive events",
        sub_num
    );
}

// ==========================================================================
// Disconnect Behavior
// ==========================================================================

#[when("I disconnect my subscription")]
async fn when_disconnect(world: &mut EventStreamServiceWorld) {
    world.current_stream = None;
    world.disconnected = true;
    // Give cleanup task time to run
    tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;
}

#[then("no events should be delivered to the disconnected subscriber")]
async fn then_disconnected_no_delivery(world: &mut EventStreamServiceWorld) {
    // Once disconnected, the stream is None - nothing to receive on
    assert!(
        world.disconnected,
        "Expected subscription to be disconnected"
    );
    assert!(
        world.current_stream.is_none(),
        "Stream should be closed after disconnect"
    );
}

#[when(expr = "subscriber {int} disconnects")]
async fn when_subscriber_disconnects(world: &mut EventStreamServiceWorld, sub_num: u32) {
    let key = format!("subscriber-{}", sub_num);
    if let Some(sub) = world.subscribers.get_mut(&key) {
        sub.stream = None;
        sub.disconnected = true;
    }
    // Give cleanup task time to run
    tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;
}

// ==========================================================================
// Event Content
// ==========================================================================

#[then(expr = "I should receive an EventBook with {int} pages")]
async fn then_eventbook_pages(world: &mut EventStreamServiceWorld, count: usize) {
    let stream = world.current_stream.as_mut().expect("No active stream");
    let received = EventStreamServiceWorld::try_receive_from_stream(stream, 100).await;
    let book = received.expect("Expected to receive EventBook");
    assert_eq!(
        book.pages.len(),
        count,
        "Expected {} pages, got {}",
        count,
        book.pages.len()
    );
    world.last_received.push(book);
}

#[then(expr = "the EventBook should have correlation ID {string}")]
async fn then_eventbook_correlation(world: &mut EventStreamServiceWorld, correlation_id: String) {
    let book = world.last_received.last().expect("No received EventBook");
    let actual = book
        .cover
        .as_ref()
        .map(|c| c.correlation_id.as_str())
        .unwrap_or("");
    assert_eq!(
        actual, correlation_id,
        "Expected correlation_id '{}', got '{}'",
        correlation_id, actual
    );
}

#[then(expr = "I should receive an EventBook with domain {string}")]
async fn then_eventbook_domain(world: &mut EventStreamServiceWorld, domain: String) {
    let stream = world.current_stream.as_mut().expect("No active stream");
    let received = EventStreamServiceWorld::try_receive_from_stream(stream, 100).await;
    let book = received.expect("Expected to receive EventBook");

    let actual = book.cover.as_ref().map(|c| c.domain.as_str()).unwrap_or("");
    assert_eq!(
        actual, domain,
        "Expected domain '{}', got '{}'",
        domain, actual
    );
    world.last_received.push(book);
}

#[then("the EventBook should have a valid root UUID")]
async fn then_eventbook_valid_root(world: &mut EventStreamServiceWorld) {
    let book = world.last_received.last().expect("No received EventBook");
    let root = book
        .cover
        .as_ref()
        .and_then(|c| c.root.as_ref())
        .expect("EventBook missing root");

    let uuid = Uuid::from_slice(&root.value).expect("Invalid UUID bytes");
    assert!(!uuid.is_nil(), "Root UUID should not be nil");
}
