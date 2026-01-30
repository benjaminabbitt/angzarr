//! Checkout command handler.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CartCheckedOut, CartState};
use common::{now, require_exists, require_not_empty, require_status_not, Result};

use crate::errmsg;
use crate::state::build_event_response;

/// Handle the Checkout command.
///
/// Finalizes the cart and marks it as checked out.
pub fn handle_checkout(
    command_book: &CommandBook,
    _command_data: &[u8],
    state: &CartState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.customer_id, errmsg::CART_NOT_FOUND)?;
    require_status_not(&state.status, "checked_out", errmsg::CART_CHECKED_OUT)?;
    require_not_empty(&state.items, errmsg::CART_EMPTY)?;

    let event = CartCheckedOut {
        final_subtotal: state.subtotal_cents,
        discount_cents: state.discount_cents,
        checked_out_at: Some(now()),
        customer_id: state.customer_id.clone(),
        items: state.items.clone(),
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.CartCheckedOut",
        event,
    ))
}
