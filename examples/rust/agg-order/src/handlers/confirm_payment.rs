//! Handler for ConfirmPayment command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{ConfirmPayment, OrderCompleted, OrderState};
use common::{decode_command, now, require_exists, require_status_not, Result};

use crate::errmsg;
use crate::state::{calculate_total, state_builder};

/// Handle the ConfirmPayment command.
pub fn handle_confirm_payment(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &OrderState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.customer_id, errmsg::ORDER_NOT_FOUND)?;
    require_status_not(&state.status, "pending", errmsg::PAYMENT_NOT_SUBMITTED)?;
    require_status_not(&state.status, "completed", errmsg::ORDER_COMPLETED)?;
    require_status_not(&state.status, "cancelled", errmsg::ORDER_CANCELLED)?;

    let cmd: ConfirmPayment = decode_command(command_data)?;

    let final_total = calculate_total(state);
    // 1 loyalty point per $1 (100 cents)
    let loyalty_points_earned = final_total / 100;

    let event = OrderCompleted {
        final_total_cents: final_total,
        payment_method: state.payment_method.clone(),
        payment_reference: cmd.payment_reference.clone(),
        loyalty_points_earned,
        completed_at: Some(now()),
        customer_root: state.customer_root.clone(),
        cart_root: state.cart_root.clone(),
        items: state.items.clone(),
        // Default to approved when no fraud service configured (router path)
        fraud_check_result: "approved".to_string(),
    };

    Ok(state_builder().build_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.OrderCompleted",
        event,
    ))
}
