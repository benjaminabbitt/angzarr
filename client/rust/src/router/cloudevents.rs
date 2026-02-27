//! CloudEvents support for Angzarr projectors.
//!
//! CloudEvents projectors transform internal domain events into CloudEvents 1.0 format
//! for external consumption via HTTP webhooks or Kafka.
//!
//! # OO Pattern (CloudEventsProjector)
//!
//! ```rust,ignore
//! pub struct PlayerCloudEventsProjector;
//!
//! impl CloudEventsProjector for PlayerCloudEventsProjector {
//!     fn name(&self) -> &str { "prj-player-cloudevents" }
//!     fn domain(&self) -> &str { "player" }
//! }
//!
//! impl PlayerCloudEventsProjector {
//!     pub fn on_player_registered(&self, event: &PlayerRegistered) -> Option<CloudEvent> {
//!         Some(CloudEvent {
//!             r#type: "com.poker.player.registered".into(),
//!             data: Some(Any::from_msg(&public).ok()?),
//!             ..Default::default()
//!         })
//!     }
//! }
//! ```
//!
//! # Functional Pattern (CloudEventsRouter)
//!
//! ```rust,ignore
//! fn handle_player_registered(event: &PlayerRegistered) -> Option<CloudEvent> {
//!     Some(CloudEvent {
//!         r#type: "com.poker.player.registered".into(),
//!         data: Some(Any::from_msg(&public).ok()?),
//!         ..Default::default()
//!     })
//! }
//!
//! let router = CloudEventsRouter::new("prj-player-cloudevents", "player")
//!     .on::<PlayerRegistered>(handle_player_registered)
//!     .on::<FundsDeposited>(handle_funds_deposited);
//!
//! run_cloudevents_projector("prj-player-cloudevents", 50091, router).await;
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use prost::Name;
use prost_types::Any;

use crate::proto::{event_page, CloudEvent, CloudEventsResponse, EventBook};

/// Handler function type for CloudEvents transformation.
pub type CloudEventsHandler<T> = fn(&T) -> Option<CloudEvent>;

/// Boxed handler for type-erased storage.
type BoxedHandler = Arc<dyn Fn(&Any) -> Option<CloudEvent> + Send + Sync>;

/// Trait for OO-style CloudEvents projectors.
///
/// Implement this trait along with `on_{event_type}` methods to create
/// a CloudEvents projector using the OO pattern.
pub trait CloudEventsProjector: Send + Sync {
    /// Get the projector name.
    fn name(&self) -> &str;

    /// Get the input domain.
    fn domain(&self) -> &str;
}

/// Functional router for CloudEvents projectors.
///
/// Provides a fluent builder API for registering event handlers that
/// transform domain events into CloudEvents.
///
/// # Example
///
/// ```rust,ignore
/// let router = CloudEventsRouter::new("prj-player-cloudevents", "player")
///     .on::<PlayerRegistered>(handle_player_registered)
///     .on::<FundsDeposited>(handle_funds_deposited);
/// ```
pub struct CloudEventsRouter {
    name: String,
    domain: String,
    handlers: HashMap<String, BoxedHandler>,
}

impl CloudEventsRouter {
    /// Create a new CloudEvents router.
    ///
    /// # Arguments
    ///
    /// * `name` - The projector name (e.g., "prj-player-cloudevents")
    /// * `domain` - The input domain (e.g., "player")
    pub fn new(name: impl Into<String>, domain: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            domain: domain.into(),
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for an event type.
    ///
    /// The event type is automatically inferred from the handler's parameter type.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The protobuf event type (must implement `prost::Name` and `prost::Message`)
    ///
    /// # Arguments
    ///
    /// * `handler` - Function that transforms the event into a CloudEvent
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// router.on::<PlayerRegistered>(|event| {
    ///     Some(CloudEvent {
    ///         r#type: "com.poker.player.registered".into(),
    ///         ..Default::default()
    ///     })
    /// })
    /// ```
    pub fn on<T>(mut self, handler: CloudEventsHandler<T>) -> Self
    where
        T: prost::Message + Name + Default + 'static,
    {
        let suffix = T::NAME;
        let boxed: BoxedHandler = Arc::new(move |any: &Any| match any.to_msg::<T>() {
            Ok(event) => handler(&event),
            Err(_) => None,
        });
        self.handlers.insert(suffix.to_string(), boxed);
        self
    }

    /// Get the projector name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the input domain.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Get the event types this router handles.
    pub fn event_types(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// Project an EventBook into CloudEvents.
    ///
    /// # Arguments
    ///
    /// * `source` - The source EventBook containing domain events
    ///
    /// # Returns
    ///
    /// A `CloudEventsResponse` containing the transformed CloudEvents.
    pub fn project(&self, source: &EventBook) -> CloudEventsResponse {
        let mut events = Vec::new();

        for page in &source.pages {
            let event_any = match &page.payload {
                Some(event_page::Payload::Event(e)) => e,
                _ => continue,
            };

            // Extract type suffix from type_url (e.g., "type.googleapis.com/examples.PlayerRegistered" -> "PlayerRegistered")
            let type_url = &event_any.type_url;
            let suffix = type_url
                .rsplit('/')
                .next()
                .and_then(|full_name: &str| full_name.rsplit('.').next())
                .unwrap_or("");

            if let Some(handler) = self.handlers.get(suffix) {
                if let Some(cloud_event) = handler(event_any) {
                    events.push(cloud_event);
                }
            }
        }

        CloudEventsResponse { events }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        let router = CloudEventsRouter::new("test-projector", "test-domain");
        assert_eq!(router.name(), "test-projector");
        assert_eq!(router.domain(), "test-domain");
    }
}
