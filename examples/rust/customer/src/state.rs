//! Customer state management and reconstruction from events.

use common::proto::{CustomerCreated, CustomerState, LoyaltyPointsAdded, LoyaltyPointsRedeemed};
use common::{make_event_book, rebuild_from_events};
use prost::Message;

use angzarr::proto::{Cover, EventBook};

/// Protobuf type URL for CustomerState snapshots.
pub const STATE_TYPE_URL: &str = "type.examples/examples.CustomerState";

/// Rebuild customer state from an event book.
///
/// Applies events in order, starting from a snapshot if present.
pub fn rebuild_state(event_book: Option<&EventBook>) -> CustomerState {
    rebuild_from_events(event_book, apply_event)
}

pub fn apply_event(state: &mut CustomerState, event: &prost_types::Any) {
    if event.type_url.ends_with("CustomerCreated") {
        if let Ok(e) = CustomerCreated::decode(event.value.as_slice()) {
            state.name = e.name;
            state.email = e.email;
        }
    } else if event.type_url.ends_with("LoyaltyPointsAdded") {
        if let Ok(e) = LoyaltyPointsAdded::decode(event.value.as_slice()) {
            // Use facts (absolute values) for idempotent state reconstruction
            state.loyalty_points = e.new_balance;
            state.lifetime_points = e.new_lifetime_points;
        }
    } else if event.type_url.ends_with("LoyaltyPointsRedeemed") {
        if let Ok(e) = LoyaltyPointsRedeemed::decode(event.value.as_slice()) {
            state.loyalty_points = e.new_balance;
        }
    }
}

/// Apply an event and build an EventBook response with updated snapshot.
pub fn build_event_response(
    state: &CustomerState,
    cover: Option<Cover>,
    next_seq: u32,
    event_type_url: &str,
    event: impl Message,
) -> EventBook {
    let event_bytes = event.encode_to_vec();
    let any = prost_types::Any {
        type_url: event_type_url.to_string(),
        value: event_bytes.clone(),
    };
    let mut new_state = state.clone();
    apply_event(&mut new_state, &any);

    make_event_book(
        cover,
        next_seq,
        event_type_url,
        event_bytes,
        STATE_TYPE_URL,
        new_state.encode_to_vec(),
    )
}
