//! Handler for ConfirmPayment command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{ConfirmPayment, OrderCompleted, OrderState};
use common::{decode_command, make_event_book, now, require_exists, require_status_not, Result};
use prost::Message;

use crate::errmsg;
use crate::state::calculate_total;

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
        customer_root: state.customer_root.clone(),
        cart_root: state.cart_root.clone(),
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.OrderCompleted",
        event.encode_to_vec(),
        "type.examples/examples.OrderState",
        new_state.encode_to_vec(),
    ))
}
