//! Handler for CancelOrder command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CancelOrder, OrderCancelled, OrderState};
use common::{decode_command, now, require_exists, require_status_not, Result};

use crate::errmsg;
use crate::state::build_event_response;

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

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.OrderCancelled",
        event,
    ))
}
