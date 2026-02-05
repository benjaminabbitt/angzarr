//! Handler for SubmitPayment command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{OrderState, PaymentSubmitted, SubmitPayment};
use common::{decode_command, now, require_exists, require_status_not, BusinessError, Result};

use crate::errmsg;
use crate::state::{build_event_response, calculate_total};

/// Handle the SubmitPayment command.
pub fn handle_submit_payment(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &OrderState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.customer_id, errmsg::ORDER_NOT_FOUND)?;
    require_status_not(
        &state.status,
        "payment_submitted",
        errmsg::PAYMENT_ALREADY_SUBMITTED,
    )?;
    require_status_not(
        &state.status,
        "completed",
        errmsg::PAYMENT_ALREADY_SUBMITTED,
    )?;
    require_status_not(&state.status, "cancelled", errmsg::ORDER_CANCELLED)?;

    let cmd: SubmitPayment = decode_command(command_data)?;

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

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.PaymentSubmitted",
        event,
    ))
}
