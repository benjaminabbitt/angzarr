//! Cart state management and event sourcing.
//!
//! Contains the state rebuild logic for reconstructing cart state from events.

use prost::Message;

use angzarr::proto::{Cover, EventBook};
use common::proto::{
    CartCheckedOut, CartCleared, CartCreated, CartItem, CartState, CouponApplied, ItemAdded,
    ItemRemoved, QuantityUpdated,
};
use common::{make_event_book, rebuild_from_events};

/// Protobuf type URL for CartState snapshots.
pub const STATE_TYPE_URL: &str = "type.examples/examples.CartState";

/// Rebuild cart state from an event book.
pub fn rebuild_state(event_book: Option<&EventBook>) -> CartState {
    rebuild_from_events(event_book, apply_event)
}

/// Apply a single event to the cart state.
///
/// Single source of truth for all cart state transitions.
pub fn apply_event(state: &mut CartState, event: &prost_types::Any) {
    if event.type_url.ends_with("CartCreated") {
        if let Ok(e) = CartCreated::decode(event.value.as_slice()) {
            state.customer_id = e.customer_id;
            state.items.clear();
            state.subtotal_cents = 0;
            state.coupon_code = String::new();
            state.discount_cents = 0;
            state.status = "active".to_string();
        }
    } else if event.type_url.ends_with("ItemAdded") {
        if let Ok(e) = ItemAdded::decode(event.value.as_slice()) {
            // Check if item already exists
            if let Some(existing) = state
                .items
                .iter_mut()
                .find(|i| i.product_id == e.product_id)
            {
                existing.quantity = e.quantity;
            } else {
                state.items.push(CartItem {
                    product_id: e.product_id,
                    name: e.name,
                    quantity: e.quantity,
                    unit_price_cents: e.unit_price_cents,
                });
            }
            state.subtotal_cents = e.new_subtotal;
        }
    } else if event.type_url.ends_with("QuantityUpdated") {
        if let Ok(e) = QuantityUpdated::decode(event.value.as_slice()) {
            if let Some(item) = state
                .items
                .iter_mut()
                .find(|i| i.product_id == e.product_id)
            {
                item.quantity = e.new_quantity;
            }
            state.subtotal_cents = e.new_subtotal;
        }
    } else if event.type_url.ends_with("ItemRemoved") {
        if let Ok(e) = ItemRemoved::decode(event.value.as_slice()) {
            state.items.retain(|i| i.product_id != e.product_id);
            state.subtotal_cents = e.new_subtotal;
        }
    } else if event.type_url.ends_with("CouponApplied") {
        if let Ok(e) = CouponApplied::decode(event.value.as_slice()) {
            state.coupon_code = e.coupon_code;
            state.discount_cents = e.discount_cents;
        }
    } else if event.type_url.ends_with("CartCleared") {
        if let Ok(e) = CartCleared::decode(event.value.as_slice()) {
            state.items.clear();
            state.subtotal_cents = e.new_subtotal;
            state.coupon_code = String::new();
            state.discount_cents = 0;
        }
    } else if event.type_url.ends_with("CartCheckedOut")
        && CartCheckedOut::decode(event.value.as_slice()).is_ok()
    {
        state.status = "checked_out".to_string();
    }
}

/// Calculate subtotal from cart items.
pub fn calculate_subtotal(items: &[CartItem]) -> i32 {
    items.iter().map(|i| i.quantity * i.unit_price_cents).sum()
}

/// Apply an event and build an EventBook response with updated snapshot.
///
/// Ensures state derivation goes through `apply_event` — single source of truth
/// for state transitions. Handlers create the event (with computed facts),
/// then delegate state derivation here.
pub fn build_event_response(
    state: &CartState,
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
