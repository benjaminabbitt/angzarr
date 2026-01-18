//! Handler for CancelOrder command.

use common::{BusinessError, Result};
use angzarr::proto::{CommandBook, EventBook};
use common::proto::{CancelOrder, OrderCancelled, OrderState};
use prost::Message;

use super::{make_event_book, now};
use crate::errmsg;

/// Handle the CancelOrder command.
pub fn handle_cancel_order(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &OrderState,
    next_seq: u32,
) -> Result<EventBook> {
    if state.customer_id.is_empty() {
        return Err(BusinessError::Rejected(errmsg::ORDER_NOT_FOUND.to_string()));
    }
    if state.status == "completed" {
        return Err(BusinessError::Rejected(errmsg::ORDER_COMPLETED.to_string()));
    }
    if state.status == "cancelled" {
        return Err(BusinessError::Rejected(errmsg::ORDER_CANCELLED.to_string()));
    }

    let cmd =
        CancelOrder::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

    let event = OrderCancelled {
        reason: cmd.reason,
        cancelled_at: Some(now()),
        loyalty_points_used: state.loyalty_points_used,
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
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.OrderCancelled",
        event.encode_to_vec(),
        new_state.encode_to_vec(),
    ))
}
