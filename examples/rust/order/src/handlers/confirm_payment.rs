//! Handler for ConfirmPayment command.

use angzarr::clients::{BusinessError, Result};
use angzarr::proto::{CommandBook, EventBook};
use common::proto::{ConfirmPayment, OrderCompleted, OrderState};
use prost::Message;

use super::{make_event_book, now};
use crate::errmsg;
use crate::state::calculate_total;

/// Handle the ConfirmPayment command.
pub fn handle_confirm_payment(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &OrderState,
    next_seq: u32,
) -> Result<EventBook> {
    if state.customer_id.is_empty() {
        return Err(BusinessError::Rejected(errmsg::ORDER_NOT_FOUND.to_string()));
    }
    if state.status == "pending" {
        return Err(BusinessError::Rejected(
            errmsg::PAYMENT_NOT_SUBMITTED.to_string(),
        ));
    }
    if state.status == "completed" {
        return Err(BusinessError::Rejected(errmsg::ORDER_COMPLETED.to_string()));
    }
    if state.status == "cancelled" {
        return Err(BusinessError::Rejected(errmsg::ORDER_CANCELLED.to_string()));
    }

    let cmd =
        ConfirmPayment::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

    let final_total = calculate_total(state);
    // 1 loyalty point per $1 (100 cents)
    let loyalty_points_earned = final_total / 100;

    let event = OrderCompleted {
        final_total_cents: final_total,
        payment_method: state.payment_method.clone(),
        payment_reference: cmd.payment_reference.clone(),
        loyalty_points_earned,
        completed_at: Some(now()),
    };

    let new_state = OrderState {
        customer_id: state.customer_id.clone(),
        items: state.items.clone(),
        subtotal_cents: state.subtotal_cents,
        discount_cents: state.discount_cents,
        loyalty_points_used: state.loyalty_points_used,
        payment_method: state.payment_method.clone(),
        payment_reference: cmd.payment_reference,
        status: "completed".to_string(),
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.OrderCompleted",
        event.encode_to_vec(),
        new_state.encode_to_vec(),
    ))
}
