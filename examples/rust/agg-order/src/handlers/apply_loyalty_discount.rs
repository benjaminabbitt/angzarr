//! Handler for ApplyLoyaltyDiscount command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{ApplyLoyaltyDiscount, LoyaltyDiscountApplied, OrderState};
use common::{decode_command, now, require_exists, BusinessError, Result};

use crate::errmsg;
use crate::state::build_event_response;

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

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.LoyaltyDiscountApplied",
        event,
    ))
}
