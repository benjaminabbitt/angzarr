//! Streaming event handler for angzarr-stream service.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};

use crate::bus::{BusError, EventHandler};
use crate::proto::event_stream_server::EventStream;
use crate::proto::{EventBook, EventStreamFilter};

type SubscriberSender = mpsc::Sender<Result<EventBook, Status>>;

/// Subscriber registration.
pub(crate) struct Subscriber {
    pub(crate) sender: SubscriberSender,
}

/// Streaming event service.
///
/// Receives events from AMQP and forwards to subscribers filtered by correlation ID.
/// Events without matching subscribers are dropped.
pub struct StreamService {
    /// Map of correlation_id -> list of subscribers
    subscriptions: Arc<RwLock<HashMap<String, Vec<Subscriber>>>>,
}

impl StreamService {
    /// Create a new stream service.
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a reference to subscriptions for the event handler.
    pub(crate) fn subscriptions(&self) -> Arc<RwLock<HashMap<String, Vec<Subscriber>>>> {
        Arc::clone(&self.subscriptions)
    }

    /// Handle an event book - forward to matching subscribers.
    ///
    /// Used by the Projector gRPC service to receive events from the projector sidecar.
    pub async fn handle(&self, book: &EventBook) {
        // Skip events without correlation ID
        let correlation_id = match book.cover.as_ref() {
            Some(c) if !c.correlation_id.is_empty() => &c.correlation_id,
            _ => {
                debug!("Dropping event without correlation_id");
                return;
            }
        };

        let mut subs = self.subscriptions.write().await;
        if let Some(subscribers) = subs.get_mut(correlation_id) {
            let mut to_remove = Vec::new();
            let mut sent_count = 0;

            for (idx, sub) in subscribers.iter().enumerate() {
                if sub.sender.is_closed() {
                    to_remove.push(idx);
                    continue;
                }

                if let Err(e) = sub.sender.try_send(Ok(book.clone())) {
                    warn!(
                        correlation_id = %correlation_id,
                        error = %e,
                        "Failed to send event to subscriber"
                    );
                    if sub.sender.is_closed() {
                        to_remove.push(idx);
                    }
                } else {
                    sent_count += 1;
                }
            }

            // Remove closed subscribers (reverse order to preserve indices)
            if !to_remove.is_empty() {
                debug!(
                    correlation_id = %correlation_id,
                    removed = to_remove.len(),
                    "Removing disconnected subscribers during event delivery"
                );
                for idx in to_remove.into_iter().rev() {
                    subscribers.remove(idx);
                }
            }

            // Remove correlation_id entry if no subscribers left
            if subscribers.is_empty() {
                subs.remove(correlation_id);
                debug!(
                    correlation_id = %correlation_id,
                    "No subscribers remaining, removed correlation entry"
                );
            } else {
                debug!(
                    correlation_id = %correlation_id,
                    sent = sent_count,
                    remaining = subscribers.len(),
                    "Event delivered to subscribers"
                );
            }
        }
    }
}

impl Default for StreamService {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl EventStream for StreamService {
    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    async fn subscribe(
        &self,
        request: Request<EventStreamFilter>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let filter = request.into_inner();

        if filter.correlation_id.is_empty() {
            return Err(Status::invalid_argument(
                "correlation_id is required for event stream subscription",
            ));
        }

        let correlation_id = filter.correlation_id.clone();
        debug!(correlation_id = %correlation_id, "New subscription registered");

        // Create channel for this subscriber
        let (tx, rx) = mpsc::channel(32);

        // Clone tx for cleanup task before moving into subscriber
        let cleanup_tx = tx.clone();

        // Register subscriber
        {
            let mut subs = self.subscriptions.write().await;
            subs.entry(correlation_id.clone())
                .or_default()
                .push(Subscriber { sender: tx });
        }

        // Spawn cleanup task that removes subscriber when channel closes
        // This triggers when the gateway drops its subscription (client disconnect or completion)
        let subscriptions = Arc::clone(&self.subscriptions);
        let cleanup_correlation_id = correlation_id.clone();
        tokio::spawn(async move {
            cleanup_tx.closed().await;
            info!(
                correlation_id = %cleanup_correlation_id,
                "Subscriber disconnected, cleaning up subscription"
            );
            let mut subs = subscriptions.write().await;
            if let Some(subscribers) = subs.get_mut(&cleanup_correlation_id) {
                let before_count = subscribers.len();
                subscribers.retain(|s| !s.sender.is_closed());
                let removed_count = before_count - subscribers.len();
                if subscribers.is_empty() {
                    subs.remove(&cleanup_correlation_id);
                    debug!(
                        correlation_id = %cleanup_correlation_id,
                        "Removed last subscriber, correlation entry cleaned up"
                    );
                } else {
                    debug!(
                        correlation_id = %cleanup_correlation_id,
                        removed = removed_count,
                        remaining = subscribers.len(),
                        "Removed closed subscribers"
                    );
                }
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}

/// Event handler that forwards to stream subscribers.
pub struct StreamEventHandler {
    subscriptions: Arc<RwLock<HashMap<String, Vec<Subscriber>>>>,
}

impl StreamEventHandler {
    /// Create handler from stream service.
    pub fn new(service: &StreamService) -> Self {
        Self {
            subscriptions: service.subscriptions(),
        }
    }
}

impl EventHandler for StreamEventHandler {
    fn handle(
        &self,
        book: Arc<EventBook>,
    ) -> futures::future::BoxFuture<'static, Result<(), BusError>> {
        let subscriptions = Arc::clone(&self.subscriptions);
        let book = Arc::clone(&book);

        Box::pin(async move {
            // Skip events without correlation ID
            let correlation_id = match book.cover.as_ref() {
                Some(c) if !c.correlation_id.is_empty() => &c.correlation_id,
                _ => {
                    debug!("Dropping event without correlation_id");
                    return Ok(());
                }
            };

            // Look up subscribers
            let mut subs = subscriptions.write().await;
            if let Some(subscribers) = subs.get_mut(correlation_id) {
                // Send to all subscribers, remove closed ones
                let mut to_remove = Vec::new();
                let mut sent_count = 0;
                for (idx, sub) in subscribers.iter().enumerate() {
                    if sub.sender.is_closed() {
                        to_remove.push(idx);
                        continue;
                    }

                    if let Err(e) = sub.sender.try_send(Ok((*book).clone())) {
                        warn!(
                            correlation_id = %correlation_id,
                            error = %e,
                            "Failed to send event to subscriber"
                        );
                        if sub.sender.is_closed() {
                            to_remove.push(idx);
                        }
                    } else {
                        sent_count += 1;
                    }
                }

                // Remove closed subscribers (reverse order to preserve indices)
                if !to_remove.is_empty() {
                    debug!(
                        correlation_id = %correlation_id,
                        removed = to_remove.len(),
                        "Removing disconnected subscribers during event delivery"
                    );
                    for idx in to_remove.into_iter().rev() {
                        subscribers.remove(idx);
                    }
                }

                // Remove correlation_id entry if no subscribers left
                if subscribers.is_empty() {
                    subs.remove(correlation_id);
                    debug!(
                        correlation_id = %correlation_id,
                        "No subscribers remaining, removed correlation entry"
                    );
                } else {
                    debug!(
                        correlation_id = %correlation_id,
                        sent = sent_count,
                        remaining = subscribers.len(),
                        "Event delivered to subscribers"
                    );
                }
            }
            // Events without subscribers are silently dropped (expected behavior)

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Cover, EventPage, Uuid as ProtoUuid};
    use tokio_stream::StreamExt;

    fn make_test_event_book(correlation_id: &str) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid {
                    value: uuid::Uuid::new_v4().as_bytes().to_vec(),
                }),
                correlation_id: correlation_id.to_string(),
            }),
            pages: vec![EventPage {
                sequence: Some(crate::proto::event_page::Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "test.Event".to_string(),
                    value: vec![],
                }),
                created_at: None,
            }],
            snapshot: None,
            snapshot_state: None,
        }
    }

    #[tokio::test]
    async fn test_subscribe_creates_subscription() {
        let service = StreamService::new();

        let filter = EventStreamFilter {
            correlation_id: "test-corr-id".to_string(),
        };

        let response = service.subscribe(Request::new(filter)).await.unwrap();
        let _stream = response.into_inner();

        // Verify subscription exists
        let subs = service.subscriptions.read().await;
        assert!(subs.contains_key("test-corr-id"));
        assert_eq!(subs.get("test-corr-id").unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_subscribe_requires_correlation_id() {
        let service = StreamService::new();

        let filter = EventStreamFilter {
            correlation_id: String::new(),
        };

        let result = service.subscribe(Request::new(filter)).await;
        match result {
            Err(status) => assert_eq!(status.code(), tonic::Code::InvalidArgument),
            Ok(_) => panic!("Expected error for empty correlation_id"),
        }
    }

    #[tokio::test]
    async fn test_subscriber_cleanup_on_disconnect() {
        let service = StreamService::new();

        let filter = EventStreamFilter {
            correlation_id: "cleanup-test".to_string(),
        };

        let response = service.subscribe(Request::new(filter)).await.unwrap();
        let stream = response.into_inner();

        // Verify subscription exists
        {
            let subs = service.subscriptions.read().await;
            assert!(subs.contains_key("cleanup-test"));
        }

        // Drop stream - simulates client disconnect
        drop(stream);

        // Give cleanup task time to run
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Verify subscription was cleaned up
        let subs = service.subscriptions.read().await;
        assert!(
            !subs.contains_key("cleanup-test"),
            "Subscription should be cleaned up after disconnect"
        );
    }

    #[tokio::test]
    async fn test_event_delivery_to_subscriber() {
        let service = StreamService::new();
        let handler = StreamEventHandler::new(&service);

        let filter = EventStreamFilter {
            correlation_id: "delivery-test".to_string(),
        };

        let response = service.subscribe(Request::new(filter)).await.unwrap();
        let mut stream = response.into_inner();

        // Deliver an event
        let book = Arc::new(make_test_event_book("delivery-test"));
        handler.handle(book).await.unwrap();

        // Verify event is received
        let received = tokio::time::timeout(tokio::time::Duration::from_millis(100), stream.next())
            .await
            .expect("Should receive event");

        assert!(received.is_some());
        let event_book = received.unwrap().unwrap();
        assert_eq!(
            event_book.cover.as_ref().unwrap().correlation_id,
            "delivery-test"
        );
    }

    #[tokio::test]
    async fn test_event_dropped_without_subscribers() {
        let service = StreamService::new();
        let handler = StreamEventHandler::new(&service);

        // No subscribers registered for this correlation ID
        let book = Arc::new(make_test_event_book("no-subscriber"));

        // Should not error - events without subscribers are silently dropped
        let result = handler.handle(book).await;
        assert!(result.is_ok());

        // Verify no subscriptions were created
        let subs = service.subscriptions.read().await;
        assert!(!subs.contains_key("no-subscriber"));
    }

    #[tokio::test]
    async fn test_closed_subscriber_removed_on_delivery() {
        let service = StreamService::new();
        let handler = StreamEventHandler::new(&service);

        let filter = EventStreamFilter {
            correlation_id: "closed-sub-test".to_string(),
        };

        let response = service.subscribe(Request::new(filter)).await.unwrap();
        let stream = response.into_inner();

        // Verify subscription exists
        {
            let subs = service.subscriptions.read().await;
            assert!(subs.contains_key("closed-sub-test"));
        }

        // Drop stream to close the receiver
        drop(stream);

        // Give a moment for the closed state to propagate
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Try to deliver an event - this should clean up the closed subscriber
        let book = Arc::new(make_test_event_book("closed-sub-test"));
        handler.handle(book).await.unwrap();

        // Verify subscription was cleaned up
        let subs = service.subscriptions.read().await;
        assert!(
            !subs.contains_key("closed-sub-test"),
            "Closed subscriber should be removed during event delivery"
        );
    }

    #[tokio::test]
    async fn test_multiple_subscribers_same_correlation() {
        let service = StreamService::new();
        let handler = StreamEventHandler::new(&service);

        let filter1 = EventStreamFilter {
            correlation_id: "multi-sub".to_string(),
        };
        let filter2 = EventStreamFilter {
            correlation_id: "multi-sub".to_string(),
        };

        let response1 = service.subscribe(Request::new(filter1)).await.unwrap();
        let response2 = service.subscribe(Request::new(filter2)).await.unwrap();
        let mut stream1 = response1.into_inner();
        let mut stream2 = response2.into_inner();

        // Verify both subscriptions exist
        {
            let subs = service.subscriptions.read().await;
            assert_eq!(subs.get("multi-sub").unwrap().len(), 2);
        }

        // Deliver an event
        let book = Arc::new(make_test_event_book("multi-sub"));
        handler.handle(book).await.unwrap();

        // Both subscribers should receive the event
        let received1 =
            tokio::time::timeout(tokio::time::Duration::from_millis(100), stream1.next())
                .await
                .expect("Subscriber 1 should receive event");

        let received2 =
            tokio::time::timeout(tokio::time::Duration::from_millis(100), stream2.next())
                .await
                .expect("Subscriber 2 should receive event");

        assert!(received1.is_some());
        assert!(received2.is_some());
    }
}
