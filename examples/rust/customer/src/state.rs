//! Customer state management and reconstruction from events.

use common::proto::{CustomerCreated, CustomerState, LoyaltyPointsAdded, LoyaltyPointsRedeemed};
use prost::Message;

use angzarr::proto::EventBook;

/// Rebuild customer state from an event book.
///
/// Applies events in order, starting from a snapshot if present.
pub fn rebuild_state(event_book: Option<&EventBook>) -> CustomerState {
    let mut state = CustomerState::default();

    let Some(book) = event_book else {
        return state;
    };

    // Start from snapshot if present
    if let Some(snapshot) = &book.snapshot {
        if let Some(snapshot_state) = &snapshot.state {
            if let Ok(s) = CustomerState::decode(snapshot_state.value.as_slice()) {
                state = s;
            }
        }
    }

    // Apply events
    for page in &book.pages {
        let Some(event) = &page.event else {
            continue;
        };

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

    state
}
