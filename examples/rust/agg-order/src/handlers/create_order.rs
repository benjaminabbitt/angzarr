//! Handler for CreateOrder command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CreateOrder, OrderCreated, OrderState};
use common::{
    decode_command, now, require_not_empty, require_not_exists, require_positive, ProtoTypeName,
    Result,
};

use crate::errmsg;
use crate::state::build_event_response;

/// Handle the CreateOrder command.
pub fn handle_create_order(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &OrderState,
    next_seq: u32,
) -> Result<EventBook> {
    require_not_exists(&state.customer_id, errmsg::ORDER_EXISTS)?;

    let cmd: CreateOrder = decode_command(command_data)?;

    require_not_empty(&cmd.items, errmsg::ITEMS_REQUIRED)?;
    for item in &cmd.items {
        require_positive(item.quantity, errmsg::QUANTITY_POSITIVE)?;
    }

    let subtotal: i32 = cmd
        .items
        .iter()
        .map(|i| i.quantity * i.unit_price_cents)
        .sum();

    let event = OrderCreated {
        customer_id: cmd.customer_id.clone(),
        items: cmd.items.clone(),
        subtotal_cents: subtotal,
        created_at: Some(now()),
        customer_root: cmd.customer_root.clone(),
        cart_root: cmd.cart_root.clone(),
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        &OrderCreated::type_url(),
        event,
    ))
}
