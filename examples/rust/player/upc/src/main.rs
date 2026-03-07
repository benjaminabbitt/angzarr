//! Player domain upcaster gRPC server.
//!
//! Transforms old event versions to current versions during replay.
//! This is a passthrough upcaster - no transformations yet.
//!
//! # Adding Transformations
//!
//! When schema evolution is needed, add transformations to the router:
//!
//! ```rust,ignore
//! use angzarr_client::UpcasterRouter;
//! use prost_types::Any;
//!
//! // Example: Transform PlayerRegisteredV1 (without ai_model_id)
//! // to PlayerRegistered (with ai_model_id defaulted to empty)
//! fn upcast_player_registered_v1(old: &Any) -> Any {
//!     // Decode old event
//!     let v1: PlayerRegisteredV1 = Any::unpack(old).unwrap();
//!
//!     // Transform to new version with defaults for new fields
//!     let current = PlayerRegistered {
//!         display_name: v1.display_name,
//!         email: v1.email,
//!         player_type: v1.player_type,
//!         ai_model_id: String::new(), // New field with default
//!         registered_at: v1.registered_at,
//!     };
//!
//!     Any::pack(&current)
//! }
//!
//! let router = UpcasterRouter::new("player")
//!     .on("PlayerRegisteredV1", upcast_player_registered_v1);
//! ```

use angzarr_client::proto::EventPage;
use angzarr_client::{run_upcaster_server, UpcasterGrpcHandler, UpcasterRouter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// docs:start:upcaster_router
/// Build the upcaster router for player domain.
///
/// Currently a passthrough - add transformations as needed for schema evolution.
fn build_router() -> UpcasterRouter {
    UpcasterRouter::new("player")
    // Example transformation (uncomment when needed):
    // .on("PlayerRegisteredV1", upcast_player_registered_v1)
}

/// Handle upcasting for player domain events.
///
/// Delegates to the router for any registered transformations.
/// Events without registered transformations pass through unchanged.
fn handle_upcast(events: &[EventPage]) -> Vec<EventPage> {
    let router = build_router();
    router.upcast(events)
}
// docs:end:upcaster_router

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // docs:start:upcaster_server
    let handler = UpcasterGrpcHandler::new("upcaster-player", "player").with_handle(handle_upcast);

    run_upcaster_server("upcaster-player", 50401, handler)
        .await
        .expect("Upcaster server failed");
    // docs:end:upcaster_server
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr_client::proto::event_page;
    use prost_types::Any;

    /// Test that events without registered transformations pass through unchanged.
    #[test]
    fn test_passthrough_no_transformation() {
        let event = Any {
            type_url: "type.googleapis.com/examples.PlayerRegistered".to_string(),
            value: vec![1, 2, 3, 4],
        };

        let page = EventPage {
            payload: Some(event_page::Payload::Event(event.clone())),
            sequence_type: Some(page_header::SequenceType::Sequence(1)),
            created_at: None,
        };

        let result = handle_upcast(&[page]);

        assert_eq!(result.len(), 1);
        if let Some(event_page::Payload::Event(e)) = &result[0].payload {
            assert_eq!(e.type_url, event.type_url);
            assert_eq!(e.value, event.value);
        } else {
            panic!("Expected event payload");
        }
    }

    /// Test that multiple events are processed in order.
    #[test]
    fn test_multiple_events_preserve_order() {
        let events: Vec<EventPage> = (0..5)
            .map(|i| EventPage {
                payload: Some(event_page::Payload::Event(Any {
                    type_url: format!("type.googleapis.com/examples.Event{}", i),
                    value: vec![i as u8],
                })),
                sequence_type: Some(page_header::SequenceType::Sequence(i)),
                created_at: None,
            })
            .collect();

        let result = handle_upcast(&events);

        assert_eq!(result.len(), 5);
        for (i, page) in result.iter().enumerate() {
            if let Some(event_page::Payload::Event(e)) = &page.payload {
                assert_eq!(
                    e.type_url,
                    format!("type.googleapis.com/examples.Event{}", i)
                );
            }
        }
    }

    /// Test that the router domain is correctly set.
    #[test]
    fn test_router_domain() {
        let router = build_router();
        assert_eq!(router.domain(), "player");
    }
}
