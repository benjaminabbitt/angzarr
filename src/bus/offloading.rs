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
                                sequence: page.sequence,
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
        use crate::proto::event_page::Payload;

        let mut new_pages = Vec::with_capacity(book.pages.len());
        let mut had_errors = false;

        for page in &book.pages {
            if let Some(Payload::External(ref reference)) = page.payload {
                // Fetch the payload
                match self.store.get(reference).await {
                    Ok(payload_bytes) => {
                        // Decode back to Any
                        match prost_types::Any::decode(payload_bytes.as_slice()) {
                            Ok(event) => {
                                new_pages.push(EventPage {
                                    sequence: page.sequence,
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
}

#[async_trait]
impl<S: PayloadStore + 'static> EventBus for OffloadingEventBus<S> {
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        // Process for offloading
        let processed = self.process_for_publish(&book).await?;
        self.inner.publish(Arc::new(processed)).await
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        // For now, just pass through - resolution happens at coordinator level
        // TODO: implement resolving handler wrapper if needed
        self.inner.subscribe(handler).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::MockEventBus;
    use crate::payload_store::FilesystemPayloadStore;
    use crate::proto::event_page;
    use tempfile::TempDir;

    async fn create_test_store() -> (Arc<FilesystemPayloadStore>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());
        (store, temp_dir)
    }

    fn make_event_book(payload_size: usize) -> EventBook {
        EventBook {
            cover: None,
            pages: vec![EventPage {
                sequence: 0,
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
}
