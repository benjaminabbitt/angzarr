//! Cart state management and event sourcing.
//!
//! Contains the state rebuild logic for reconstructing cart state from events.

use prost::Message;

use angzarr::proto::EventBook;
use common::proto::{
    CartCheckedOut, CartCleared, CartCreated, CartItem, CartState, CouponApplied, ItemAdded,
    ItemRemoved, QuantityUpdated,
};

/// Rebuild cart state from an event book.
///
/// Applies events in order, starting from any snapshot if present.
pub fn rebuild_state(event_book: Option<&EventBook>) -> CartState {
    let mut state = CartState::default();

    let Some(book) = event_book else {
        return state;
    };

    // Start from snapshot if present
    if let Some(snapshot) = &book.snapshot {
        if let Some(snapshot_state) = &snapshot.state {
            if let Ok(s) = CartState::decode(snapshot_state.value.as_slice()) {
                state = s;
            }
        }
    }

    // Apply events
    for page in &book.pages {
        let Some(event) = &page.event else {
            continue;
        };

        apply_event(&mut state, event);
    }

    state
}

/// Apply a single event to the cart state.
fn apply_event(state: &mut CartState, event: &prost_types::Any) {
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

/// Get current timestamp.
pub fn now() -> prost_types::Timestamp {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    prost_types::Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}
