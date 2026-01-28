//! Handler for CancelOrder command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CancelOrder, OrderCancelled, OrderState};
use common::{decode_command, make_event_book, now, require_exists, require_status_not, Result};
use prost::Message;

use crate::errmsg;

/// Handle the CancelOrder command.
pub fn handle_cancel_order(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &OrderState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.customer_id, errmsg::ORDER_NOT_FOUND)?;
    require_status_not(&state.status, "completed", errmsg::ORDER_COMPLETED)?;
    require_status_not(&state.status, "cancelled", errmsg::ORDER_CANCELLED)?;

    let cmd: CancelOrder = decode_command(command_data)?;

    let event = OrderCancelled {
        reason: cmd.reason,
        cancelled_at: Some(now()),
        loyalty_points_used: state.loyalty_points_used,
        customer_root: state.customer_root.clone(),
        items: state.items.clone(),
        cart_root: state.cart_root.clone(),
    };

    let new_state = OrderState {
        customer_id: state.customer_id.clone(),
        items: state.items.clone(),
        subtotal_cents: state.subtotal_cents,
        discount_cents: state.discount_cents,
        loyalty_points_used: state.loyalty_points_used,
        payment_method: state.payment_method.clone(),
        payment_reference: state.payment_reference.clone(),
        status: "cancelled".to_string(),
        customer_root: state.customer_root.clone(),
        cart_root: state.cart_root.clone(),
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.OrderCancelled",
        event.encode_to_vec(),
        "type.examples/examples.OrderState",
        new_state.encode_to_vec(),
    ))
}
