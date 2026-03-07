//! Event version transformation via UpcasterRouter.
//!
//! Upcasters transform old event versions to current versions during replay.
//! They enable schema evolution without breaking existing event stores.
//!
//! # Example
//!
//! ```rust,ignore
//! use angzarr_client::UpcasterRouter;
//! use prost_types::Any;
//!
//! fn upcast_order_created_v1(old: &Any) -> Any {
//!     let v1: OrderCreatedV1 = old.unpack().unwrap();
//!     Any::pack(&OrderCreated {
//!         order_id: v1.order_id,
//!         customer_id: v1.customer_id,
//!         total: 0, // New field with default
//!     })
//! }
//!
//! let router = UpcasterRouter::new("order")
//!     .on("OrderCreatedV1", upcast_order_created_v1)
//!     .on("OrderShippedV1", upcast_order_shipped_v1);
//!
//! let new_events = router.upcast(&old_events);
//! ```

use prost_types::Any;

use crate::proto::{event_page, EventPage};

#[cfg(test)]
use crate::proto::{page_header::SequenceType, PageHeader};

/// Handler function for upcasting old events to new versions.
///
/// Takes a reference to the old event (packed as Any) and returns
/// the new event (also packed as Any).
pub type UpcasterHandler = fn(&Any) -> Any;

/// Boxed handler for dynamic upcasting (allows closures).
pub type BoxedUpcasterHandler = Box<dyn Fn(&Any) -> Any + Send + Sync>;

struct UpcasterEntry {
    /// Type URL suffix to match (e.g., "OrderCreatedV1")
    suffix: String,
    /// Handler function to transform the event
    handler: BoxedUpcasterHandler,
}

/// Router for transforming old event versions to current versions.
///
/// Events matching registered handlers are transformed.
/// Events without matching handlers pass through unchanged.
///
/// # Example
///
/// ```rust,ignore
/// let router = UpcasterRouter::new("order")
///     .on("OrderCreatedV1", |old| {
///         let v1: OrderCreatedV1 = old.unpack().unwrap();
///         Any::pack(&OrderCreated {
///             order_id: v1.order_id,
///             // ... transform fields ...
///         })
///     });
///
/// let new_events = router.upcast(&old_events);
/// ```
pub struct UpcasterRouter {
    domain: String,
    handlers: Vec<UpcasterEntry>,
}

impl UpcasterRouter {
    /// Create a new upcaster router for a domain.
    ///
    /// # Arguments
    ///
    /// * `domain` - The domain this upcaster handles
    pub fn new(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            handlers: Vec::new(),
        }
    }

    /// Register a handler for an old event type.
    ///
    /// The suffix is matched against the end of the event's type_url.
    /// For example, suffix "OrderCreatedV1" matches
    /// "type.googleapis.com/examples.OrderCreatedV1".
    ///
    /// # Arguments
    ///
    /// * `suffix` - The type_url suffix to match
    /// * `handler` - Function that transforms old event to new event
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// router.on("OrderCreatedV1", |old| {
    ///     let v1: OrderCreatedV1 = old.unpack().unwrap();
    ///     Any::pack(&OrderCreated { ... })
    /// });
    /// ```
    pub fn on<F>(mut self, suffix: impl Into<String>, handler: F) -> Self
    where
        F: Fn(&Any) -> Any + Send + Sync + 'static,
    {
        self.handlers.push(UpcasterEntry {
            suffix: suffix.into(),
            handler: Box::new(handler),
        });
        self
    }

    /// Register a handler using a function pointer.
    ///
    /// This is a convenience method for registering simple function pointers.
    pub fn on_fn(self, suffix: impl Into<String>, handler: UpcasterHandler) -> Self {
        self.on(suffix, handler)
    }

    /// Get the domain this upcaster handles.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Get the type URL suffixes this upcaster handles.
    pub fn event_types(&self) -> Vec<String> {
        self.handlers.iter().map(|e| e.suffix.clone()).collect()
    }

    /// Transform a single event to current version.
    ///
    /// Returns the transformed event if a handler matches,
    /// or a clone of the original if no transformation needed.
    pub fn upcast_event(&self, event: &Any) -> Any {
        let type_url = &event.type_url;

        for entry in &self.handlers {
            if type_url.ends_with(&entry.suffix) {
                return (entry.handler)(event);
            }
        }

        // No transformation needed - return clone
        event.clone()
    }

    /// Transform a list of events to current versions.
    ///
    /// Events matching registered handlers are transformed.
    /// Events without matching handlers pass through unchanged.
    ///
    /// # Arguments
    ///
    /// * `events` - Slice of EventPages to transform
    ///
    /// # Returns
    ///
    /// Vec of EventPages with transformed events
    pub fn upcast(&self, events: &[EventPage]) -> Vec<EventPage> {
        events
            .iter()
            .map(|page| {
                match &page.payload {
                    Some(event_page::Payload::Event(event)) => {
                        let new_event = self.upcast_event(event);

                        // Only create new page if event was transformed
                        if new_event.type_url != event.type_url {
                            EventPage {
                                payload: Some(event_page::Payload::Event(new_event)),
                                header: page.header.clone(),
                                created_at: page.created_at,
                            }
                        } else {
                            page.clone()
                        }
                    }
                    // Non-event payloads pass through unchanged
                    _ => page.clone(),
                }
            })
            .collect()
    }

    /// Check if this upcaster has a handler for the given type URL.
    pub fn handles(&self, type_url: &str) -> bool {
        self.handlers.iter().any(|e| type_url.ends_with(&e.suffix))
    }
}

/// Mode marker for upcaster routers.
pub struct UpcasterMode;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upcaster_router_creation() {
        let router = UpcasterRouter::new("order");
        assert_eq!(router.domain(), "order");
        assert!(router.event_types().is_empty());
    }

    #[test]
    fn upcaster_router_registration() {
        let router = UpcasterRouter::new("order")
            .on("OrderCreatedV1", |old| old.clone())
            .on("OrderShippedV1", |old| old.clone());

        assert_eq!(router.event_types().len(), 2);
        assert!(router.handles("type.googleapis.com/examples.OrderCreatedV1"));
        assert!(router.handles("type.googleapis.com/examples.OrderShippedV1"));
        assert!(!router.handles("type.googleapis.com/examples.OrderCompleted"));
    }

    #[test]
    fn upcaster_passthrough_no_match() {
        let router = UpcasterRouter::new("order").on("OrderCreatedV1", |old| old.clone());

        let event = Any {
            type_url: "type.googleapis.com/examples.OrderCompleted".to_string(),
            value: vec![1, 2, 3],
        };

        let result = router.upcast_event(&event);
        assert_eq!(result.type_url, event.type_url);
        assert_eq!(result.value, event.value);
    }

    #[test]
    fn upcaster_transforms_matching() {
        let router = UpcasterRouter::new("order").on("OrderCreatedV1", |_old| Any {
            type_url: "type.googleapis.com/examples.OrderCreated".to_string(),
            value: vec![4, 5, 6],
        });

        let event = Any {
            type_url: "type.googleapis.com/examples.OrderCreatedV1".to_string(),
            value: vec![1, 2, 3],
        };

        let result = router.upcast_event(&event);
        assert_eq!(result.type_url, "type.googleapis.com/examples.OrderCreated");
        assert_eq!(result.value, vec![4, 5, 6]);
    }

    #[test]
    fn upcaster_batch_processing() {
        let router = UpcasterRouter::new("order").on("OrderCreatedV1", |_old| Any {
            type_url: "type.googleapis.com/examples.OrderCreated".to_string(),
            value: vec![],
        });

        let pages = vec![
            EventPage {
                payload: Some(event_page::Payload::Event(Any {
                    type_url: "type.googleapis.com/examples.OrderCreatedV1".to_string(),
                    value: vec![],
                })),
                header: Some(PageHeader {
                    sequence_type: Some(SequenceType::Sequence(0)),
                }),
                created_at: None,
            },
            EventPage {
                payload: Some(event_page::Payload::Event(Any {
                    type_url: "type.googleapis.com/examples.OrderCompleted".to_string(),
                    value: vec![],
                })),
                header: Some(PageHeader {
                    sequence_type: Some(SequenceType::Sequence(1)),
                }),
                created_at: None,
            },
        ];

        let result = router.upcast(&pages);
        assert_eq!(result.len(), 2);

        // First event should be transformed
        if let Some(event_page::Payload::Event(e)) = &result[0].payload {
            assert_eq!(e.type_url, "type.googleapis.com/examples.OrderCreated");
        } else {
            panic!("Expected event payload");
        }

        // Second event should pass through
        if let Some(event_page::Payload::Event(e)) = &result[1].payload {
            assert_eq!(e.type_url, "type.googleapis.com/examples.OrderCompleted");
        } else {
            panic!("Expected event payload");
        }
    }
}
