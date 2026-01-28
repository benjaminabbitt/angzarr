//! Clear cart command handler.

use prost::Message;

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CartCleared, CartState};
use common::{make_event_book, now, require_exists, require_status_not, Result};

use crate::errmsg;

/// Handle the ClearCart command.
///
/// Removes all items from the cart and resets coupon.
pub fn handle_clear_cart(
    command_book: &CommandBook,
    _command_data: &[u8],
    state: &CartState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.customer_id, errmsg::CART_NOT_FOUND)?;
    require_status_not(&state.status, "checked_out", errmsg::CART_CHECKED_OUT)?;

    let event = CartCleared {
        new_subtotal: 0,
        cleared_at: Some(now()),
        items: state.items.clone(),
    };

    let new_state = CartState {
        customer_id: state.customer_id.clone(),
        items: vec![],
        subtotal_cents: 0,
        coupon_code: String::new(),
        discount_cents: 0,
        status: state.status.clone(),
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.CartCleared",
        event.encode_to_vec(),
        "type.examples/examples.CartState",
        new_state.encode_to_vec(),
    ))
}
