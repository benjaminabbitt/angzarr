//! Handler for SubmitPayment command.

use common::{BusinessError, Result};
use angzarr::proto::{CommandBook, EventBook};
use common::proto::{OrderState, PaymentSubmitted, SubmitPayment};
use prost::Message;

use super::{make_event_book, now};
use crate::errmsg;
use crate::state::calculate_total;

/// Handle the SubmitPayment command.
pub fn handle_submit_payment(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &OrderState,
    next_seq: u32,
) -> Result<EventBook> {
    if state.customer_id.is_empty() {
        return Err(BusinessError::Rejected(errmsg::ORDER_NOT_FOUND.to_string()));
    }
    if state.status == "payment_submitted" || state.status == "completed" {
        return Err(BusinessError::Rejected(
            errmsg::PAYMENT_ALREADY_SUBMITTED.to_string(),
        ));
    }
    if state.status == "cancelled" {
        return Err(BusinessError::Rejected(errmsg::ORDER_CANCELLED.to_string()));
    }

    let cmd =
        SubmitPayment::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

    let expected_total = calculate_total(state);
    if cmd.amount_cents != expected_total {
        return Err(BusinessError::Rejected(format!(
            "{}: expected {}, got {}",
            errmsg::PAYMENT_AMOUNT_MISMATCH,
            expected_total,
            cmd.amount_cents
        )));
    }

    let event = PaymentSubmitted {
        payment_method: cmd.payment_method.clone(),
        amount_cents: cmd.amount_cents,
        submitted_at: Some(now()),
    };

    let new_state = OrderState {
        customer_id: state.customer_id.clone(),
        items: state.items.clone(),
        subtotal_cents: state.subtotal_cents,
        discount_cents: state.discount_cents,
        loyalty_points_used: state.loyalty_points_used,
        payment_method: cmd.payment_method,
        payment_reference: state.payment_reference.clone(),
        status: "payment_submitted".to_string(),
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.PaymentSubmitted",
        event.encode_to_vec(),
        new_state.encode_to_vec(),
    ))
}
