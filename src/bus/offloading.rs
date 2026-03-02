//! Transparent event bus wrapper that offloads large payloads to external storage.
//!
//! Implements the claim check pattern: when an event payload exceeds the bus's
//! size limit, the payload is stored externally and replaced with a reference.
//! On the receiving side, references are resolved back to full payloads.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use angzarr::bus::{OffloadingEventBus, OffloadingConfig};
//! use angzarr::payload_store::FilesystemPayloadStore;
//!
//! let store = FilesystemPayloadStore::new("/var/angzarr/payloads").await?;
//! let config = OffloadingConfig::new(store)
//!     .with_threshold(256 * 1024);  // 256 KB threshold
//!
//! let bus = OffloadingEventBus::wrap(inner_bus, config);
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use prost::Message;
use tracing::{debug, warn};

use super::{BusError, EventBus, EventHandler, PublishResult, Result};
use crate::payload_store::{PayloadStore, PayloadStoreError};
use crate::proto::{EventBook, EventPage};

/// Configuration for payload offloading.
pub struct OffloadingConfig<S: PayloadStore> {
    /// The payload store to use for offloading.
    pub store: Arc<S>,
    /// Minimum payload size to trigger offloading.
    /// Default: use bus's max_message_size() if available.
    pub threshold: Option<usize>,
}

impl<S: PayloadStore> OffloadingConfig<S> {
    /// Create new offloading config with the given payload store.
    pub fn new(store: Arc<S>) -> Self {
        Self {
            store,
            threshold: None,
        }
    }

    /// Set explicit threshold for offloading (overrides bus limit).
    pub fn with_threshold(mut self, bytes: usize) -> Self {
        self.threshold = Some(bytes);
        self
    }
}

/// Transparent event bus wrapper that offloads large payloads.
///
/// Wraps any EventBus and automatically:
/// - On publish: offloads large event payloads to external storage
/// - On subscribe: resolves payload references back to full events
pub struct OffloadingEventBus<S: PayloadStore> {
    inner: Arc<dyn EventBus>,
    store: Arc<S>,
    threshold: Option<usize>,
}

impl<S: PayloadStore + 'static> OffloadingEventBus<S> {
    /// Wrap an event bus with payload offloading.
    pub fn wrap(inner: Arc<dyn EventBus>, config: OffloadingConfig<S>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            store: config.store,
            threshold: config.threshold,
        })
    }

    /// Get effective threshold for this bus.
    fn effective_threshold(&self) -> Option<usize> {
        self.threshold.or_else(|| self.inner.max_message_size())
    }

    /// Process an event book for publishing, offloading large payloads.
    async fn process_for_publish(&self, book: &EventBook) -> Result<EventBook> {
        let threshold = match self.effective_threshold() {
            Some(t) => t,
            None => return Ok(book.clone()), // No limit, pass through
        };

        // Check total serialized size first
        let total_size = book.encoded_len();
        if total_size <= threshold {
            return Ok(book.clone()); // Small enough, pass through
        }

        // Need to offload - process each page
        use crate::proto::event_page::Payload;

        let mut new_pages = Vec::with_capacity(book.pages.len());

        for page in &book.pages {
            let page_size = page.encoded_len();

            // Only offload pages that are large
            if page_size > threshold / 2 {
                // Offload if page is >50% of threshold
                if let Some(Payload::Event(ref event)) = page.payload {
                    let payload_bytes = event.encode_to_vec();

                    match self.store.put(&payload_bytes).await {
                        Ok(reference) => {
                            debug!(
                                original_size = payload_bytes.len(),
                                uri = %reference.uri,
                                "Offloaded large event payload"
                            );

                            new_pages.push(EventPage {
                                sequence_type: page.sequence_type.clone(),
                                created_at: page.created_at,
                                payload: Some(Payload::External(reference)),
                            });
                            continue;
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to offload payload, sending inline");
                        }
                    }
                }
            }

            // Keep original page (small or offload failed)
            new_pages.push(page.clone());
        }

        Ok(EventBook {
            cover: book.cover.clone(),
            pages: new_pages,
            snapshot: book.snapshot.clone(),
            next_sequence: book.next_sequence,
        })
    }

    /// Resolve external payload references in an event book.
    pub async fn resolve_payloads(&self, book: &EventBook) -> Result<EventBook> {
        resolve_payloads_with_store(self.store.as_ref(), book).await
    }
}

#[async_trait]
impl<S: PayloadStore + 'static> EventBus for OffloadingEventBus<S> {
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        // Process for offloading
        let processed = self.process_for_publish(&book).await?;
        self.inner.publish(Arc::new(processed)).await
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        // Wrap handler to resolve external payloads before delivery.
        // This makes offloading transparent to consumers - they receive
        // fully resolved EventBooks regardless of whether payloads were offloaded.
        let resolving_handler = Box::new(ResolvingHandler {
            inner: Arc::from(handler),
            store: Arc::clone(&self.store),
        });
        self.inner.subscribe(resolving_handler).await
    }

    async fn start_consuming(&self) -> Result<()> {
        self.inner.start_consuming().await
    }

    async fn create_subscriber(
        &self,
        name: &str,
        domain_filter: Option<&str>,
    ) -> Result<Arc<dyn EventBus>> {
        // Create inner subscriber and wrap it
        let inner_sub = self.inner.create_subscriber(name, domain_filter).await?;
        Ok(Arc::new(OffloadingEventBus {
            inner: inner_sub,
            store: Arc::clone(&self.store),
            threshold: self.threshold,
        }))
    }

    fn max_message_size(&self) -> Option<usize> {
        self.inner.max_message_size()
    }
}

impl From<PayloadStoreError> for BusError {
    fn from(e: PayloadStoreError) -> Self {
        BusError::Publish(format!("Payload store error: {}", e))
    }
}

// ============================================================================
// Resolving Handler
// ============================================================================

/// Resolve external payload references in an EventBook.
///
/// Standalone function for use by `ResolvingHandler`. Fetches external payloads
/// from the store and replaces references with inline events.
async fn resolve_payloads_with_store<S: PayloadStore>(
    store: &S,
    book: &EventBook,
) -> Result<EventBook> {
    use crate::proto::event_page::Payload;

    let mut new_pages = Vec::with_capacity(book.pages.len());
    let mut had_errors = false;

    for page in &book.pages {
        if let Some(Payload::External(ref reference)) = page.payload {
            // Fetch the payload
            match store.get(reference).await {
                Ok(payload_bytes) => {
                    // Decode back to Any
                    match prost_types::Any::decode(payload_bytes.as_slice()) {
                        Ok(event) => {
                            new_pages.push(EventPage {
                                sequence_type: page.sequence_type.clone(),
                                created_at: page.created_at,
                                payload: Some(Payload::Event(event)),
                            });
                            continue;
                        }
                        Err(e) => {
                            warn!(
                                uri = %reference.uri,
                                error = %e,
                                "Failed to decode retrieved payload"
                            );
                            had_errors = true;
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        uri = %reference.uri,
                        error = %e,
                        "Failed to retrieve external payload"
                    );
                    had_errors = true;
                }
            }
        }

        // Keep original page (no reference or resolution failed)
        new_pages.push(page.clone());
    }

    if had_errors {
        // Log but don't fail - partial resolution is better than nothing
        warn!("Some external payloads could not be resolved");
    }

    Ok(EventBook {
        cover: book.cover.clone(),
        pages: new_pages,
        snapshot: book.snapshot.clone(),
        next_sequence: book.next_sequence,
    })
}

/// Handler wrapper that resolves external payload references before delegation.
///
/// When the `OffloadingEventBus` receives events with external payload references,
/// this handler fetches the actual payloads from the store before passing them
/// to the wrapped handler. This makes payload offloading transparent to consumers.
struct ResolvingHandler<S: PayloadStore> {
    inner: Arc<dyn EventHandler>,
    store: Arc<S>,
}

impl<S: PayloadStore + 'static> EventHandler for ResolvingHandler<S> {
    fn handle(
        &self,
        book: Arc<EventBook>,
    ) -> BoxFuture<'static, std::result::Result<(), BusError>> {
        let store = Arc::clone(&self.store);
        let inner = Arc::clone(&self.inner);

        Box::pin(async move {
            // Resolve external payloads before passing to the actual handler
            let resolved = resolve_payloads_with_store(store.as_ref(), &book).await?;
            inner.handle(Arc::new(resolved)).await
        })
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the payload offloading event bus wrapper.
    //!
    //! The offloading bus implements the "claim check" pattern: large event
    //! payloads are stored externally and replaced with references. This
    //! enables bus transports with size limits (e.g., Kafka's 1MB default)
    //! to handle arbitrarily large events.
    //!
    //! Key behaviors:
    //! - Small payloads pass through unchanged (no storage overhead)
    //! - Large payloads are offloaded and replaced with External references
    //! - References are resolved back to inline events on receive
    //! - The offloading is transparent to handlers
    //!
    //! Without offloading, large aggregates (e.g., with embedded documents)
    //! would fail to publish, breaking event sourcing entirely.

    use super::*;
    use crate::bus::MockEventBus;
    use crate::payload_store::FilesystemPayloadStore;
    use crate::proto::event_page;
    use tempfile::TempDir;

    // ============================================================================
    // Test Helpers
    // ============================================================================

    async fn create_test_store() -> (Arc<FilesystemPayloadStore>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());
        (store, temp_dir)
    }

    fn make_event_book(payload_size: usize) -> EventBook {
        EventBook {
            cover: None,
            pages: vec![EventPage {
                sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(0)),
                created_at: None,
                payload: Some(event_page::Payload::Event(prost_types::Any {
                    type_url: "test.Event".to_string(),
                    value: vec![0u8; payload_size],
                })),
            }],
            snapshot: None,
            next_sequence: 1,
        }
    }

    // ============================================================================
    // Offloading Tests
    // ============================================================================

    /// Small payloads below threshold pass through without offloading.
    ///
    /// Offloading adds latency (store write) and storage cost. Events that
    /// fit within bus limits should be sent inline for efficiency.
    #[tokio::test]
    async fn test_small_payload_passes_through() {
        let (store, _temp) = create_test_store().await;
        let mock_bus = Arc::new(MockEventBus::new());
        let inner: Arc<dyn EventBus> = Arc::clone(&mock_bus) as Arc<dyn EventBus>;
        let config = OffloadingConfig::new(store).with_threshold(1024);
        let bus = OffloadingEventBus::wrap(inner, config);

        let book = make_event_book(100); // Small payload
        bus.publish(Arc::new(book.clone())).await.unwrap();

        // Should have been published without offloading
        let published = mock_bus.take_published().await;
        assert_eq!(published.len(), 1);
        assert!(matches!(
            &published[0].pages[0].payload,
            Some(event_page::Payload::Event(_))
        ));
    }

    /// Large payloads above threshold are replaced with External references.
    ///
    /// The actual payload is stored externally; the bus message contains only
    /// a URI reference. This keeps bus messages small regardless of event size.
    #[tokio::test]
    async fn test_large_payload_gets_offloaded() {
        let (store, _temp) = create_test_store().await;
        let mock_bus = Arc::new(MockEventBus::new());
        let inner: Arc<dyn EventBus> = Arc::clone(&mock_bus) as Arc<dyn EventBus>;
        let config = OffloadingConfig::new(store).with_threshold(100);
        let bus = OffloadingEventBus::wrap(inner, config);

        let book = make_event_book(500); // Large payload
        bus.publish(Arc::new(book.clone())).await.unwrap();

        // Should have been offloaded
        let published = mock_bus.take_published().await;
        assert_eq!(published.len(), 1);
        assert!(matches!(
            &published[0].pages[0].payload,
            Some(event_page::Payload::External(_))
        ));
    }

    /// External references resolve back to original payload.
    ///
    /// Round-trip integrity: offload → publish → receive → resolve produces
    /// the original event. Handlers see fully-resolved EventBooks.
    #[tokio::test]
    async fn test_resolve_external_payload() {
        let (store, _temp) = create_test_store().await;
        let mock_bus = Arc::new(MockEventBus::new());
        let inner: Arc<dyn EventBus> = Arc::clone(&mock_bus) as Arc<dyn EventBus>;
        let config = OffloadingConfig::new(Arc::clone(&store)).with_threshold(100);
        let bus = OffloadingEventBus::wrap(inner, config);

        // Create and publish large event
        let original = make_event_book(500);
        bus.publish(Arc::new(original.clone())).await.unwrap();

        // Get the offloaded version
        let published = mock_bus.take_published().await;
        let offloaded = &published[0];

        // Resolve the payload
        let resolved = bus.resolve_payloads(offloaded).await.unwrap();

        // Should have event restored
        let resolved_event = match &resolved.pages[0].payload {
            Some(event_page::Payload::Event(e)) => e,
            _ => panic!("Expected resolved event payload"),
        };
        let original_event = match &original.pages[0].payload {
            Some(event_page::Payload::Event(e)) => e,
            _ => panic!("Expected original event payload"),
        };
        assert_eq!(original_event.type_url, resolved_event.type_url);
        assert_eq!(original_event.value.len(), resolved_event.value.len());
    }

    /// No threshold means no offloading — all events pass through.
    ///
    /// When the inner bus has no max_message_size and no explicit threshold
    /// is configured, offloading is disabled. Used for buses without limits.
    #[tokio::test]
    async fn test_no_threshold_passes_all() {
        let (store, _temp) = create_test_store().await;
        let mock_bus = Arc::new(MockEventBus::new());
        let inner: Arc<dyn EventBus> = Arc::clone(&mock_bus) as Arc<dyn EventBus>;
        let config = OffloadingConfig::new(store); // No threshold set
        let bus = OffloadingEventBus::wrap(inner, config);

        let book = make_event_book(10000); // Very large
        bus.publish(Arc::new(book.clone())).await.unwrap();

        // Should pass through since inner bus has no limit
        let published = mock_bus.take_published().await;
        assert_eq!(published.len(), 1);
        assert!(matches!(
            &published[0].pages[0].payload,
            Some(event_page::Payload::Event(_))
        ));
    }

    // ============================================================================
    // ResolvingHandler Tests
    // ============================================================================
    //
    // The ResolvingHandler wraps user handlers and transparently resolves
    // External references before delivery. This makes offloading invisible
    // to business logic.

    /// Test handler that captures received EventBooks for verification.
    struct CapturingHandler {
        received: Arc<tokio::sync::RwLock<Vec<EventBook>>>,
    }

    impl CapturingHandler {
        fn new() -> (Self, Arc<tokio::sync::RwLock<Vec<EventBook>>>) {
            let received = Arc::new(tokio::sync::RwLock::new(Vec::new()));
            (
                Self {
                    received: Arc::clone(&received),
                },
                received,
            )
        }
    }

    impl EventHandler for CapturingHandler {
        fn handle(
            &self,
            book: Arc<EventBook>,
        ) -> BoxFuture<'static, std::result::Result<(), BusError>> {
            let received = Arc::clone(&self.received);
            Box::pin(async move {
                received.write().await.push((*book).clone());
                Ok(())
            })
        }
    }

    /// External payloads are resolved before handler receives event.
    ///
    /// Handler sees inline Event, not External reference. The business logic
    /// doesn't need to know about offloading — it's transparent.
    #[tokio::test]
    async fn test_resolving_handler_resolves_external_payloads() {
        let (store, _temp) = create_test_store().await;

        // Create an offloaded event by storing payload externally
        let original_event = prost_types::Any {
            type_url: "test.Event".to_string(),
            value: vec![42u8; 500],
        };
        let payload_bytes = original_event.encode_to_vec();
        let reference = store.put(&payload_bytes).await.unwrap();

        // Create EventBook with external reference
        let offloaded_book = EventBook {
            cover: None,
            pages: vec![EventPage {
                sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(0)),
                created_at: None,
                payload: Some(event_page::Payload::External(reference)),
            }],
            snapshot: None,
            next_sequence: 1,
        };

        // Set up resolving handler
        let (capturing_handler, received) = CapturingHandler::new();
        let resolving_handler = ResolvingHandler {
            inner: Arc::new(capturing_handler),
            store: Arc::clone(&store),
        };

        // Invoke the resolving handler with offloaded event
        resolving_handler
            .handle(Arc::new(offloaded_book))
            .await
            .unwrap();

        // Verify inner handler received resolved event
        let captured = received.read().await;
        assert_eq!(captured.len(), 1);
        let resolved_payload = &captured[0].pages[0].payload;
        match resolved_payload {
            Some(event_page::Payload::Event(e)) => {
                assert_eq!(e.type_url, "test.Event");
                assert_eq!(e.value.len(), 500);
                assert!(e.value.iter().all(|&b| b == 42));
            }
            _ => panic!(
                "Expected resolved Event payload, got {:?}",
                resolved_payload
            ),
        }
    }

    /// Inline events pass through without modification.
    ///
    /// Events that were never offloaded (small payloads) should be delivered
    /// unchanged. Resolution is a no-op for inline events.
    #[tokio::test]
    async fn test_resolving_handler_passes_inline_events_unchanged() {
        let (store, _temp) = create_test_store().await;

        // Create EventBook with inline event (no external reference)
        let inline_book = make_event_book(100);

        // Set up resolving handler
        let (capturing_handler, received) = CapturingHandler::new();
        let resolving_handler = ResolvingHandler {
            inner: Arc::new(capturing_handler),
            store: Arc::clone(&store),
        };

        // Invoke the resolving handler
        resolving_handler
            .handle(Arc::new(inline_book.clone()))
            .await
            .unwrap();

        // Verify inner handler received event unchanged
        let captured = received.read().await;
        assert_eq!(captured.len(), 1);
        match &captured[0].pages[0].payload {
            Some(event_page::Payload::Event(e)) => {
                assert_eq!(e.type_url, "test.Event");
                assert_eq!(e.value.len(), 100);
            }
            _ => panic!("Expected inline Event payload"),
        }
    }
}
