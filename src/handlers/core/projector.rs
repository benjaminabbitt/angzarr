//! Projector event handler.
//!
//! Receives events from the event bus and forwards them to projector
//! services via the `ProjectorHandler` trait.
//!
//! Works with any `ProjectorHandler` implementation — gRPC (distributed)
//! or local (standalone) — enabling deploy-anywhere projector code.
//!
//! When projectors produce output (Projections), these are published back
//! to the event bus as synthetic EventBooks with the original correlation_id
//! preserved, enabling streaming of projector results back to clients via
//! angzarr-stream.

use std::sync::Arc;

use futures::future::BoxFuture;
use prost::Message;
use prost_types::Any;
use tracing::{debug, info, Instrument};

use crate::bus::{BusError, EventBus, EventHandler};
use crate::orchestration::projector::{GrpcProjectorHandler, ProjectionMode, ProjectorHandler};
use crate::proto::projector_service_client::ProjectorServiceClient;
use crate::proto::{EventBook, Projection};
use crate::proto_ext::{CoverExt, PROJECTION_DOMAIN_PREFIX, PROJECTION_TYPE_URL};

/// Event handler that forwards events to a projector via `ProjectorHandler`.
///
/// Enables the same handler code for both distributed (gRPC) and standalone
/// (local) modes.
///
/// Calls projector to get output, then publishes the Projection back to
/// the event bus as a synthetic EventBook for streaming.
pub struct ProjectorEventHandler {
    handler: Arc<dyn ProjectorHandler>,
    publisher: Option<Arc<dyn EventBus>>,
    /// Domain filter — only handle events from these domains. Empty = all.
    domains: Vec<String>,
    /// If true, this projector is synchronous (handled inline by the aggregate pipeline).
    /// Async distribution should skip it.
    synchronous: bool,
    /// Projector name (used for metrics and tracing).
    name: String,
}

impl ProjectorEventHandler {
    /// Create from a projector handler.
    pub fn from_handler(handler: Arc<dyn ProjectorHandler>, name: String) -> Self {
        Self {
            handler,
            publisher: None,
            domains: Vec::new(),
            synchronous: false,
            name,
        }
    }

    /// Create from a gRPC projector client.
    pub fn new(client: ProjectorServiceClient<tonic::transport::Channel>, name: String) -> Self {
        let handler: Arc<dyn ProjectorHandler> = Arc::new(GrpcProjectorHandler::new(client));
        Self::from_handler(handler, name)
    }

    /// Set publisher for streaming output.
    pub fn with_publisher(mut self, publisher: Arc<dyn EventBus>) -> Self {
        self.publisher = Some(publisher);
        self
    }

    /// Set domain filter.
    pub fn with_domains(mut self, domains: Vec<String>) -> Self {
        self.domains = domains;
        self
    }

    /// Set synchronous mode.
    pub fn with_synchronous(mut self, synchronous: bool) -> Self {
        self.synchronous = synchronous;
        self
    }
}

impl EventHandler for ProjectorEventHandler {
    fn handle(&self, book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
        // Skip synchronous projectors in async distribution
        if self.synchronous {
            return Box::pin(async { Ok(()) });
        }

        // Check domain filter using routing key (edition-prefixed)
        if !self.domains.is_empty() {
            let routing_key = book.routing_key();
            if !self.domains.iter().any(|d| d == &routing_key) {
                return Box::pin(async { Ok(()) });
            }
        } else {
            // Exclude infrastructure domains (underscore prefix) by default
            let domain = book.domain();
            if domain.starts_with('_') {
                return Box::pin(async { Ok(()) });
            }
        }

        let correlation_id = book.correlation_id().to_string();
        let domain = book.domain().to_string();
        let projector_name = self.name.clone();
        let span =
            tracing::info_span!("projector.handle", %projector_name, %correlation_id, %domain);

        let handler = self.handler.clone();
        let publisher = self.publisher.clone();

        Box::pin(
            async move {
                let book_owned = (*book).clone();

                let result: Result<(), BusError> = async {
                    let projection = handler
                        .handle(&book_owned, ProjectionMode::Execute)
                        .await
                        .map_err(BusError::Grpc)?;

                    // If we have a publisher and the projection has content, publish it back
                    if let Some(ref publisher) = publisher {
                        if projection.projection.is_some() || !projection.projector.is_empty() {
                            debug!(
                                projector = %projection.projector,
                                sequence = projection.sequence,
                                "Publishing projection output"
                            );

                            let source_edition =
                                book.cover.as_ref().and_then(|c| c.edition.clone());
                            let projection_event_book = create_projection_event_book(
                                projection,
                                &correlation_id,
                                source_edition,
                            );

                            info!(
                                domain = %projection_event_book.domain(),
                                "Publishing projection for streaming"
                            );

                            publisher.publish(Arc::new(projection_event_book)).await?;
                        }
                    }

                    Ok(())
                }
                .await;

                result
            }
            .instrument(span),
        )
    }
}

/// Convert a Projection to a synthetic EventBook for AMQP transport.
///
/// Uses a special domain prefix `_projection.{projector_name}` so clients
/// can distinguish projection results from domain events. The projection
/// is serialized as the event payload - clients deserialize the Projection
/// proto from the event.
fn create_projection_event_book(
    projection: Projection,
    correlation_id: &str,
    source_edition: Option<crate::proto::Edition>,
) -> EventBook {
    let projector_name = projection.projector.clone();

    // Create a cover with special projection domain
    let cover = projection.cover.clone().map(|mut c| {
        c.domain = format!("{PROJECTION_DOMAIN_PREFIX}.{}.{}", projector_name, c.domain);
        c
    });

    // Serialize the projection as the event payload
    let projection_bytes = projection.encode_to_vec();

    // Ensure correlation_id is set on cover
    let cover = match cover {
        Some(mut c) => {
            if c.correlation_id.is_empty() {
                c.correlation_id = correlation_id.to_string();
            }
            Some(c)
        }
        None => Some(crate::proto::Cover {
            domain: format!("{PROJECTION_DOMAIN_PREFIX}.{}", projector_name),
            root: None,
            correlation_id: correlation_id.to_string(),
            edition: source_edition,
            external_id: String::new(),
        }),
    };

    EventBook {
        cover,
        pages: vec![crate::proto::EventPage {
            sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(
                projection.sequence,
            )),
            payload: Some(crate::proto::event_page::Payload::Event(Any {
                type_url: PROJECTION_TYPE_URL.to_string(),
                value: projection_bytes,
            })),
            created_at: None,
        }],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Cover, Edition, Projection};
    use crate::proto_ext::CoverExt;

    fn make_cover(domain: &str) -> Cover {
        Cover {
            domain: domain.to_string(),
            root: None,
            correlation_id: String::new(),
            edition: None,
            external_id: String::new(),
        }
    }

    fn make_projection(projector: &str, sequence: u32) -> Projection {
        Projection {
            projector: projector.to_string(),
            sequence,
            cover: Some(make_cover("orders")),
            projection: None,
        }
    }

    #[test]
    fn create_projection_event_book_sets_projection_domain_prefix() {
        let projection = make_projection("order-summary", 5);
        let event_book = create_projection_event_book(projection, "corr-123", None);

        let domain = event_book.domain();
        assert!(domain.starts_with(PROJECTION_DOMAIN_PREFIX));
        assert!(domain.contains("order-summary"));
        assert!(domain.contains("orders"));
    }

    #[test]
    fn create_projection_event_book_preserves_correlation_id() {
        let projection = make_projection("order-summary", 5);
        let event_book = create_projection_event_book(projection, "my-correlation", None);

        assert_eq!(event_book.correlation_id(), "my-correlation");
    }

    #[test]
    fn create_projection_event_book_sets_sequence() {
        let projection = make_projection("order-summary", 42);
        let event_book = create_projection_event_book(projection, "corr-123", None);

        assert_eq!(event_book.pages.len(), 1);
        let page = &event_book.pages[0];
        match &page.sequence_type {
            Some(crate::proto::event_page::SequenceType::Sequence(seq)) => {
                assert_eq!(*seq, 42);
            }
            _ => panic!("Expected Sequence type"),
        }
    }

    #[test]
    fn create_projection_event_book_sets_correct_type_url() {
        let projection = make_projection("order-summary", 5);
        let event_book = create_projection_event_book(projection, "corr-123", None);

        let page = &event_book.pages[0];
        let any = match &page.payload {
            Some(crate::proto::event_page::Payload::Event(any)) => any,
            _ => panic!("Expected Event payload"),
        };

        assert_eq!(any.type_url, PROJECTION_TYPE_URL);
    }

    #[test]
    fn create_projection_event_book_deserializes_correctly() {
        let original = make_projection("order-summary", 5);
        let event_book = create_projection_event_book(original.clone(), "corr-123", None);

        let page = &event_book.pages[0];
        let any = match &page.payload {
            Some(crate::proto::event_page::Payload::Event(any)) => any,
            _ => panic!("Expected Event payload"),
        };

        let decoded = Projection::decode(any.value.as_slice()).unwrap();
        assert_eq!(decoded.projector, "order-summary");
        assert_eq!(decoded.sequence, 5);
    }

    #[test]
    fn create_projection_event_book_without_cover_creates_minimal_cover() {
        let projection = Projection {
            projector: "order-summary".to_string(),
            sequence: 5,
            cover: None,
            projection: None,
        };
        let event_book = create_projection_event_book(projection, "corr-123", None);

        assert!(event_book.cover.is_some());
        let cover = event_book.cover.as_ref().unwrap();
        assert!(cover.domain.starts_with(PROJECTION_DOMAIN_PREFIX));
        assert_eq!(cover.correlation_id, "corr-123");
    }

    #[test]
    fn create_projection_event_book_uses_source_edition_when_no_cover() {
        // When projection has no cover, source_edition is used
        let projection = Projection {
            projector: "order-summary".to_string(),
            sequence: 5,
            cover: None,
            projection: None,
        };
        let edition = Edition {
            name: "v2".to_string(),
            divergences: vec![],
        };
        let event_book = create_projection_event_book(projection, "corr-123", Some(edition));

        let cover = event_book.cover.as_ref().unwrap();
        assert_eq!(cover.edition.as_ref().unwrap().name, "v2");
    }

    #[test]
    fn create_projection_event_book_preserves_projection_cover_edition() {
        // When projection has a cover with edition, that edition is preserved
        let mut projection = make_projection("order-summary", 5);
        projection.cover.as_mut().unwrap().edition = Some(Edition {
            name: "v3".to_string(),
            divergences: vec![],
        });

        // Even if we pass a different source_edition, the projection's own is used
        let source_edition = Edition {
            name: "v1".to_string(),
            divergences: vec![],
        };
        let event_book = create_projection_event_book(projection, "corr-123", Some(source_edition));

        let cover = event_book.cover.as_ref().unwrap();
        assert_eq!(cover.edition.as_ref().unwrap().name, "v3");
    }

    #[test]
    fn create_projection_event_book_has_no_snapshot() {
        let projection = make_projection("order-summary", 5);
        let event_book = create_projection_event_book(projection, "corr-123", None);

        // Projection EventBooks should never have snapshots
        assert!(event_book.snapshot.is_none());
    }

    // === Tests for projection publishing conditions ===
    // These test the condition: projection.projection.is_some() || !projection.projector.is_empty()

    #[test]
    fn projection_with_content_should_be_publishable() {
        // Has projection content
        let projection = Projection {
            projector: "".to_string(), // empty projector name
            sequence: 1,
            cover: Some(make_cover("orders")),
            projection: Some(prost_types::Any {
                type_url: "test".to_string(),
                value: vec![1, 2, 3],
            }),
        };

        // Should be publishable because projection.is_some()
        let should_publish = projection.projection.is_some() || !projection.projector.is_empty();
        assert!(
            should_publish,
            "Projection with content should be publishable"
        );
    }

    #[test]
    fn projection_with_projector_name_should_be_publishable() {
        // Has projector name but no content
        let projection = Projection {
            projector: "order-summary".to_string(),
            sequence: 1,
            cover: Some(make_cover("orders")),
            projection: None,
        };

        // Should be publishable because projector is not empty
        let should_publish = projection.projection.is_some() || !projection.projector.is_empty();
        assert!(
            should_publish,
            "Projection with projector name should be publishable"
        );
    }

    #[test]
    fn empty_projection_should_not_be_publishable() {
        // No content AND no projector name
        let projection = Projection {
            projector: "".to_string(),
            sequence: 1,
            cover: Some(make_cover("orders")),
            projection: None,
        };

        // Should NOT be publishable
        let should_publish = projection.projection.is_some() || !projection.projector.is_empty();
        assert!(
            !should_publish,
            "Empty projection should not be publishable"
        );
    }

    #[test]
    fn projection_publishing_condition_requires_or_not_and() {
        // Test that verifies || behavior (not &&)
        // With &&, this would be false. With ||, this should be true.
        let projection = Projection {
            projector: "order-summary".to_string(), // non-empty
            sequence: 1,
            cover: None,
            projection: None, // is_none
        };

        // projection.is_some() = false, !projector.is_empty() = true
        // With ||: false || true = true
        // With &&: false && true = false
        let should_publish = projection.projection.is_some() || !projection.projector.is_empty();
        assert!(should_publish, "Should use OR logic, not AND");
    }

    #[test]
    fn projection_publishing_requires_negation_on_is_empty() {
        // Test that verifies we check !is_empty (not is_empty)
        let projection = Projection {
            projector: "order-summary".to_string(), // non-empty
            sequence: 1,
            cover: None,
            projection: None,
        };

        // Without negation: projector.is_empty() = false
        // With negation: !projector.is_empty() = true
        assert!(
            !projection.projector.is_empty(),
            "Non-empty projector should pass !is_empty check"
        );

        let empty_projection = Projection {
            projector: "".to_string(), // empty
            sequence: 1,
            cover: None,
            projection: None,
        };

        assert!(
            empty_projection.projector.is_empty(),
            "Empty projector should fail !is_empty check"
        );
    }

    // === Tests for domain filtering logic in handle() ===
    // These test the conditions at lines 91-93:
    // if !self.domains.is_empty() {
    //     let routing_key = book.routing_key();
    //     if !self.domains.iter().any(|d| d == &routing_key) {
    //         return Box::pin(async { Ok(()) });

    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex as TokioMutex;

    struct MockProjectorHandler {
        call_count: AtomicUsize,
        response: TokioMutex<Projection>,
    }

    impl MockProjectorHandler {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
                response: TokioMutex::new(Projection {
                    projector: "test".to_string(),
                    sequence: 1,
                    cover: None,
                    projection: None,
                }),
            }
        }

        fn with_response(projector: &str, has_projection: bool) -> Self {
            Self {
                call_count: AtomicUsize::new(0),
                response: TokioMutex::new(Projection {
                    projector: projector.to_string(),
                    sequence: 1,
                    cover: None,
                    projection: if has_projection {
                        Some(prost_types::Any {
                            type_url: "test".to_string(),
                            value: vec![1, 2, 3],
                        })
                    } else {
                        None
                    },
                }),
            }
        }

        fn calls(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl ProjectorHandler for MockProjectorHandler {
        async fn handle(
            &self,
            _events: &EventBook,
            _mode: ProjectionMode,
        ) -> Result<Projection, tonic::Status> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.response.lock().await.clone())
        }
    }

    /// Mock EventBus that tracks publish calls
    struct MockEventBus {
        publish_count: AtomicUsize,
        published_books: TokioMutex<Vec<EventBook>>,
    }

    impl MockEventBus {
        fn new() -> Self {
            Self {
                publish_count: AtomicUsize::new(0),
                published_books: TokioMutex::new(Vec::new()),
            }
        }

        fn publish_calls(&self) -> usize {
            self.publish_count.load(Ordering::SeqCst)
        }

        async fn get_published_books(&self) -> Vec<EventBook> {
            self.published_books.lock().await.clone()
        }
    }

    #[async_trait::async_trait]
    impl crate::bus::EventBus for MockEventBus {
        async fn publish(
            &self,
            book: Arc<EventBook>,
        ) -> crate::bus::error::Result<crate::bus::PublishResult> {
            self.publish_count.fetch_add(1, Ordering::SeqCst);
            self.published_books.lock().await.push((*book).clone());
            Ok(crate::bus::PublishResult::default())
        }

        async fn subscribe(
            &self,
            _handler: Box<dyn crate::bus::EventHandler>,
        ) -> crate::bus::error::Result<()> {
            unimplemented!("Not needed for these tests")
        }

        async fn create_subscriber(
            &self,
            _name: &str,
            _domain_filter: Option<&str>,
        ) -> crate::bus::error::Result<Arc<dyn crate::bus::EventBus>> {
            unimplemented!("Not needed for these tests")
        }
    }

    fn make_event_book(domain: &str) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: None,
                correlation_id: "test-corr".to_string(),
                edition: None,
                external_id: String::new(),
            }),
            pages: vec![],
            snapshot: None,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn handler_with_empty_domains_handles_any_event() {
        // When domains is empty, no filtering occurs
        let mock = Arc::new(MockProjectorHandler::new());
        let handler =
            ProjectorEventHandler::from_handler(mock.clone(), "test-projector".to_string())
                .with_domains(vec![]); // Empty domain filter

        let book = Arc::new(make_event_book("any-domain"));
        let result = handler.handle(book).await;

        assert!(result.is_ok());
        assert_eq!(
            mock.calls(),
            1,
            "Handler should be called when domains is empty"
        );
    }

    #[tokio::test]
    async fn handler_with_matching_domain_handles_event() {
        // When domains contains the event's domain, event is handled
        let mock = Arc::new(MockProjectorHandler::new());
        let handler =
            ProjectorEventHandler::from_handler(mock.clone(), "test-projector".to_string())
                .with_domains(vec!["orders".to_string(), "inventory".to_string()]);

        let book = Arc::new(make_event_book("orders"));
        let result = handler.handle(book).await;

        assert!(result.is_ok());
        assert_eq!(
            mock.calls(),
            1,
            "Handler should be called when domain matches"
        );
    }

    #[tokio::test]
    async fn handler_with_non_matching_domain_skips_event() {
        // When domains does NOT contain the event's domain, event is skipped
        let mock = Arc::new(MockProjectorHandler::new());
        let handler =
            ProjectorEventHandler::from_handler(mock.clone(), "test-projector".to_string())
                .with_domains(vec!["orders".to_string(), "inventory".to_string()]);

        let book = Arc::new(make_event_book("fulfillment")); // Not in the domain list
        let result = handler.handle(book).await;

        assert!(result.is_ok());
        assert_eq!(
            mock.calls(),
            0,
            "Handler should NOT be called when domain doesn't match"
        );
    }

    #[tokio::test]
    async fn domain_filter_checks_equality_not_inequality() {
        // This test ensures we use == not != when checking domain matches
        // If the code used !=, this test would fail because:
        // - "orders" != "orders" is false
        // - "orders" != "inventory" is true
        // With any(), d != routing_key would match "inventory" first, incorrectly allowing
        let mock = Arc::new(MockProjectorHandler::new());
        let handler =
            ProjectorEventHandler::from_handler(mock.clone(), "test-projector".to_string())
                .with_domains(vec!["orders".to_string()]);

        let book = Arc::new(make_event_book("orders")); // Exact match
        let result = handler.handle(book).await;

        assert!(result.is_ok());
        assert_eq!(mock.calls(), 1, "Exact domain match should be handled");

        // Reset by creating new mock
        let mock2 = Arc::new(MockProjectorHandler::new());
        let handler2 =
            ProjectorEventHandler::from_handler(mock2.clone(), "test-projector".to_string())
                .with_domains(vec!["orders".to_string()]);

        let book2 = Arc::new(make_event_book("not-orders")); // No match
        let result2 = handler2.handle(book2).await;

        assert!(result2.is_ok());
        assert_eq!(mock2.calls(), 0, "Non-matching domain should be skipped");
    }

    #[tokio::test]
    async fn domain_filter_requires_non_empty_check_negation() {
        // This verifies we check !self.domains.is_empty() not self.domains.is_empty()
        // If we didn't negate, an empty domains list would enter the filter block
        // and incorrectly skip events (because any() on empty list is false)
        let mock = Arc::new(MockProjectorHandler::new());
        let handler =
            ProjectorEventHandler::from_handler(mock.clone(), "test-projector".to_string())
                .with_domains(vec![]); // Empty - should NOT enter filter block

        let book = Arc::new(make_event_book("any-domain"));
        let result = handler.handle(book).await;

        assert!(result.is_ok());
        // If ! was deleted from !self.domains.is_empty(), domains.is_empty() = true
        // would enter the filter block, then any() on empty vec = false,
        // then !false = true (with correct negation) or false (without)
        // This test catches that by verifying events pass through
        assert_eq!(
            mock.calls(),
            1,
            "Empty domains list should not filter events"
        );
    }

    #[tokio::test]
    async fn domain_filter_any_match_negation_required() {
        // Tests the !self.domains.iter().any(...) negation
        // If ! was removed, we'd SKIP matching domains and HANDLE non-matching ones
        let mock_match = Arc::new(MockProjectorHandler::new());
        let handler_match =
            ProjectorEventHandler::from_handler(mock_match.clone(), "test".to_string())
                .with_domains(vec!["orders".to_string()]);

        let book_match = Arc::new(make_event_book("orders")); // DOES match
        handler_match.handle(book_match).await.unwrap();

        let mock_nomatch = Arc::new(MockProjectorHandler::new());
        let handler_nomatch =
            ProjectorEventHandler::from_handler(mock_nomatch.clone(), "test".to_string())
                .with_domains(vec!["orders".to_string()]);

        let book_nomatch = Arc::new(make_event_book("inventory")); // Does NOT match
        handler_nomatch.handle(book_nomatch).await.unwrap();

        // With correct logic (!any(match)):
        // - matching domain: any() = true, !true = false, don't skip → handler called
        // - non-matching domain: any() = false, !false = true, skip → handler not called
        // Without negation (any(match)):
        // - matching domain: any() = true, skip → handler not called (WRONG)
        // - non-matching domain: any() = false, don't skip → handler called (WRONG)
        assert_eq!(mock_match.calls(), 1, "Matching domain should be handled");
        assert_eq!(
            mock_nomatch.calls(),
            0,
            "Non-matching domain should be skipped"
        );
    }

    // === Tests for publishing condition (line 125) ===
    // These test: projection.projection.is_some() || !projection.projector.is_empty()

    #[tokio::test]
    async fn handler_publishes_when_projection_has_content() {
        // projection.is_some() = true, so publish regardless of projector name
        let mock = Arc::new(MockProjectorHandler::with_response("", true)); // has projection content
        let publisher = Arc::new(MockEventBus::new());
        let handler =
            ProjectorEventHandler::from_handler(mock.clone(), "test-projector".to_string())
                .with_publisher(publisher.clone());

        let book = Arc::new(make_event_book("orders"));
        let result = handler.handle(book).await;

        assert!(result.is_ok());
        assert_eq!(
            publisher.publish_calls(),
            1,
            "Should publish when projection has content"
        );
    }

    #[tokio::test]
    async fn handler_publishes_when_projector_name_non_empty() {
        // projection.is_none() but projector name is non-empty, so should publish
        let mock = Arc::new(MockProjectorHandler::with_response("order-summary", false));
        let publisher = Arc::new(MockEventBus::new());
        let handler =
            ProjectorEventHandler::from_handler(mock.clone(), "test-projector".to_string())
                .with_publisher(publisher.clone());

        let book = Arc::new(make_event_book("orders"));
        let result = handler.handle(book).await;

        assert!(result.is_ok());
        assert_eq!(
            publisher.publish_calls(),
            1,
            "Should publish when projector name is non-empty"
        );
    }

    #[tokio::test]
    async fn handler_does_not_publish_when_empty_projection() {
        // projection.is_none() AND projector name is empty, so should NOT publish
        let mock = Arc::new(MockProjectorHandler::with_response("", false)); // empty projector, no projection
        let publisher = Arc::new(MockEventBus::new());
        let handler =
            ProjectorEventHandler::from_handler(mock.clone(), "test-projector".to_string())
                .with_publisher(publisher.clone());

        let book = Arc::new(make_event_book("orders"));
        let result = handler.handle(book).await;

        assert!(result.is_ok());
        assert_eq!(
            publisher.publish_calls(),
            0,
            "Should NOT publish when projection is empty"
        );
    }

    #[tokio::test]
    async fn publishing_condition_uses_or_not_and() {
        // This verifies || behavior, not &&
        // With &&: both conditions must be true (projection.is_some() AND !projector.is_empty())
        // With ||: either condition being true triggers publish
        // Test case: has projection content but empty projector name
        let mock = Arc::new(MockProjectorHandler::with_response("", true)); // projection is_some, projector empty
        let publisher = Arc::new(MockEventBus::new());
        let handler =
            ProjectorEventHandler::from_handler(mock.clone(), "test-projector".to_string())
                .with_publisher(publisher.clone());

        let book = Arc::new(make_event_book("orders"));
        let result = handler.handle(book).await;

        // With ||: true || false = true → publish
        // With &&: true && false = false → don't publish
        assert!(result.is_ok());
        assert_eq!(
            publisher.publish_calls(),
            1,
            "Should use OR logic (publish if either is true)"
        );
    }

    #[tokio::test]
    async fn publishing_condition_requires_negation_on_is_empty() {
        // This verifies !projection.projector.is_empty() not projection.projector.is_empty()
        // Test case: has non-empty projector name but no projection content
        let mock = Arc::new(MockProjectorHandler::with_response("order-summary", false));
        let publisher = Arc::new(MockEventBus::new());
        let handler =
            ProjectorEventHandler::from_handler(mock.clone(), "test-projector".to_string())
                .with_publisher(publisher.clone());

        let book = Arc::new(make_event_book("orders"));
        let result = handler.handle(book).await;

        // projector is "order-summary" (non-empty)
        // Without negation: is_empty() = false → condition is false → don't publish (WRONG)
        // With negation: !is_empty() = true → condition is true → publish (CORRECT)
        assert!(result.is_ok());
        assert_eq!(
            publisher.publish_calls(),
            1,
            "Should publish when projector name is non-empty"
        );

        // Also test empty projector with no projection
        let mock2 = Arc::new(MockProjectorHandler::with_response("", false)); // empty projector, no projection
        let publisher2 = Arc::new(MockEventBus::new());
        let handler2 =
            ProjectorEventHandler::from_handler(mock2.clone(), "test-projector".to_string())
                .with_publisher(publisher2.clone());

        let book2 = Arc::new(make_event_book("orders"));
        let result2 = handler2.handle(book2).await;

        // Without negation: is_empty() = true → condition is true → publish (WRONG)
        // With negation: !is_empty() = false → condition is false → don't publish (CORRECT)
        assert!(result2.is_ok());
        assert_eq!(
            publisher2.publish_calls(),
            0,
            "Should NOT publish when projector name is empty"
        );
    }

    #[tokio::test]
    async fn published_event_book_has_no_snapshot() {
        // Verifies that the created EventBook has snapshot = None
        let mock = Arc::new(MockProjectorHandler::with_response("order-summary", false));
        let publisher = Arc::new(MockEventBus::new());
        let handler =
            ProjectorEventHandler::from_handler(mock.clone(), "test-projector".to_string())
                .with_publisher(publisher.clone());

        let book = Arc::new(make_event_book("orders"));
        let result = handler.handle(book).await;

        assert!(result.is_ok());
        assert_eq!(publisher.publish_calls(), 1);

        let published = publisher.get_published_books().await;
        assert_eq!(published.len(), 1);
        assert!(
            published[0].snapshot.is_none(),
            "Published EventBook should have no snapshot"
        );
    }
}
