//! Handler for ApplyLoyaltyDiscount command.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{ApplyLoyaltyDiscount, LoyaltyDiscountApplied, OrderState};
use common::{
    decode_command, now, require_exists, require_positive, require_status, BusinessError,
    ProtoTypeName, Result,
};

use crate::errmsg;
use crate::state::state_builder;
use crate::status::OrderStatus;

/// Handle the ApplyLoyaltyDiscount command.
pub fn handle_apply_loyalty_discount(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &OrderState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.customer_id, errmsg::ORDER_NOT_FOUND)?;
    require_status(
        &state.status,
        OrderStatus::Pending.as_str(),
        errmsg::ORDER_NOT_PENDING,
    )?;
    if state.loyalty_points_used > 0 {
        return Err(BusinessError::Rejected(
            errmsg::LOYALTY_ALREADY_APPLIED.to_string(),
        ));
    }

    let cmd: ApplyLoyaltyDiscount = decode_command(command_data)?;

    require_positive(cmd.points, errmsg::POINTS_POSITIVE)?;
    require_positive(cmd.discount_cents, errmsg::DISCOUNT_POSITIVE)?;
    if cmd.discount_cents > state.subtotal_cents {
        return Err(BusinessError::Rejected(
            errmsg::DISCOUNT_EXCEEDS_SUBTOTAL.to_string(),
        ));
    }

    let event = LoyaltyDiscountApplied {
        points_used: cmd.points,
        discount_cents: cmd.discount_cents,
        applied_at: Some(now()),
    };

    Ok(state_builder().build_response(
        state,
        command_book.cover.clone(),
        next_seq,
        &LoyaltyDiscountApplied::type_url(),
        event,
    ))
}
