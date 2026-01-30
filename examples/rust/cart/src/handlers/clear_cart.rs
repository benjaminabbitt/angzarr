//! Clear cart command handler.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CartCleared, CartState};
use common::{now, require_exists, require_status_not, Result};

use crate::errmsg;
use crate::state::build_event_response;

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

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.CartCleared",
        event,
    ))
}
