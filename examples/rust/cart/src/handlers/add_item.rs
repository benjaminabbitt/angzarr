//! Add item command handler.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{AddItem, CartItem, CartState, ItemAdded};
use common::{
    decode_command, now, require_exists, require_positive, require_status_not, Result,
};

use crate::errmsg;
use crate::state::{build_event_response, calculate_subtotal};

/// Handle the AddItem command.
///
/// Adds an item to the cart or increases quantity if already present.
pub fn handle_add_item(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &CartState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.customer_id, errmsg::CART_NOT_FOUND)?;
    require_status_not(&state.status, "checked_out", errmsg::CART_CHECKED_OUT)?;

    let cmd: AddItem = decode_command(command_data)?;

    require_positive(cmd.quantity, errmsg::QUANTITY_POSITIVE)?;

    // Calculate new quantity (add to existing if present)
    let existing_qty = state
        .items
        .iter()
        .find(|i| i.product_id == cmd.product_id)
        .map(|i| i.quantity)
        .unwrap_or(0);
    let new_quantity = existing_qty + cmd.quantity;

    // Calculate new subtotal
    let mut items = state.items.clone();
    if let Some(item) = items.iter_mut().find(|i| i.product_id == cmd.product_id) {
        item.quantity = new_quantity;
    } else {
        items.push(CartItem {
            product_id: cmd.product_id.clone(),
            name: cmd.name.clone(),
            quantity: cmd.quantity,
            unit_price_cents: cmd.unit_price_cents,
        });
    }
    let new_subtotal = calculate_subtotal(&items);

    let event = ItemAdded {
        product_id: cmd.product_id.clone(),
        name: cmd.name.clone(),
        quantity: new_quantity,
        unit_price_cents: cmd.unit_price_cents,
        new_subtotal,
        added_at: Some(now()),
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.ItemAdded",
        event,
    ))
}
