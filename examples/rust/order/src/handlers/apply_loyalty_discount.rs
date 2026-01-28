//! Handler for ApplyLoyaltyDiscount command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{ApplyLoyaltyDiscount, LoyaltyDiscountApplied, OrderState};
use common::{decode_command, make_event_book, now, require_exists, BusinessError, Result};
use prost::Message;

use crate::errmsg;

/// Handle the ApplyLoyaltyDiscount command.
pub fn handle_apply_loyalty_discount(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &OrderState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.customer_id, errmsg::ORDER_NOT_FOUND)?;
    if state.loyalty_points_used > 0 {
        return Err(BusinessError::Rejected(
            errmsg::LOYALTY_ALREADY_APPLIED.to_string(),
        ));
    }

    let cmd: ApplyLoyaltyDiscount = decode_command(command_data)?;

    let event = LoyaltyDiscountApplied {
        points_used: cmd.points,
        discount_cents: cmd.discount_cents,
        applied_at: Some(now()),
    };

    let new_state = OrderState {
        customer_id: state.customer_id.clone(),
        items: state.items.clone(),
        subtotal_cents: state.subtotal_cents,
        discount_cents: cmd.discount_cents,
        loyalty_points_used: cmd.points,
        payment_method: state.payment_method.clone(),
        payment_reference: state.payment_reference.clone(),
        status: state.status.clone(),
        customer_root: state.customer_root.clone(),
        cart_root: state.cart_root.clone(),
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.LoyaltyDiscountApplied",
        event.encode_to_vec(),
        "type.examples/examples.OrderState",
        new_state.encode_to_vec(),
    ))
}
