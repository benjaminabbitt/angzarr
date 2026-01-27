//! Handler for ApplyLoyaltyDiscount command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{ApplyLoyaltyDiscount, LoyaltyDiscountApplied, OrderState};
use common::{BusinessError, Result};
use prost::Message;

use super::{make_event_book, now};
use crate::errmsg;

/// Handle the ApplyLoyaltyDiscount command.
pub fn handle_apply_loyalty_discount(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &OrderState,
    next_seq: u32,
) -> Result<EventBook> {
    if state.customer_id.is_empty() {
        return Err(BusinessError::Rejected(errmsg::ORDER_NOT_FOUND.to_string()));
    }
    if state.loyalty_points_used > 0 {
        return Err(BusinessError::Rejected(
            errmsg::LOYALTY_ALREADY_APPLIED.to_string(),
        ));
    }

    let cmd = ApplyLoyaltyDiscount::decode(command_data)
        .map_err(|e| BusinessError::Rejected(e.to_string()))?;

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
    };

    Ok(make_event_book(
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.LoyaltyDiscountApplied",
        event.encode_to_vec(),
        new_state.encode_to_vec(),
    ))
}
