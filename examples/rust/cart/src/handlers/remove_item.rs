//! Remove item command handler.

use prost::Message;

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CartItem, CartState, ItemRemoved, RemoveItem};
use common::{
    decode_command, make_event_book, now, require_exists, require_status_not, BusinessError, Result,
};

use crate::errmsg;
use crate::state::calculate_subtotal;

/// Handle the RemoveItem command.
///
/// Removes an item from the cart entirely.
pub fn handle_remove_item(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &CartState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.customer_id, errmsg::CART_NOT_FOUND)?;
    require_status_not(&state.status, "checked_out", errmsg::CART_CHECKED_OUT)?;

    let cmd: RemoveItem = decode_command(command_data)?;

    let item = state
        .items
        .iter()
        .find(|i| i.product_id == cmd.product_id)
        .ok_or_else(|| BusinessError::Rejected(errmsg::ITEM_NOT_IN_CART.to_string()))?;

    let removed_quantity = item.quantity;

    // Calculate new subtotal
    let items: Vec<CartItem> = state
        .items
        .iter()
        .filter(|i| i.product_id != cmd.product_id)
        .cloned()
        .collect();
    let new_subtotal = calculate_subtotal(&items);

    let event = ItemRemoved {
        product_id: cmd.product_id,
        quantity: removed_quantity,
        new_subtotal,
        removed_at: Some(now()),
    };

    let new_state = CartState {
        customer_id: state.customer_id.clone(),
        items,
        subtotal_cents: new_subtotal,
        coupon_code: state.coupon_code.clone(),
        discount_cents: state.discount_cents,
        status: state.status.clone(),
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.ItemRemoved",
        event.encode_to_vec(),
        "type.examples/examples.CartState",
        new_state.encode_to_vec(),
    ))
}
