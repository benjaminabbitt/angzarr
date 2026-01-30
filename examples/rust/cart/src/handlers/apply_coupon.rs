//! Apply coupon command handler.

use angzarr::proto::{CommandBook, EventBook};
use common::proto::{ApplyCoupon, CartState, CouponApplied};
use common::{
    decode_command, now, require_exists, require_not_empty, require_not_exists, require_status_not,
    Result,
};

use crate::errmsg;
use crate::state::build_event_response;

/// Handle the ApplyCoupon command.
///
/// Applies a coupon to the cart. Supports percentage and fixed discounts.
pub fn handle_apply_coupon(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &CartState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.customer_id, errmsg::CART_NOT_FOUND)?;
    require_status_not(&state.status, "checked_out", errmsg::CART_CHECKED_OUT)?;
    require_not_exists(&state.coupon_code, errmsg::COUPON_ALREADY_APPLIED)?;
    require_not_empty(&state.items, errmsg::CART_EMPTY)?;

    let cmd: ApplyCoupon = decode_command(command_data)?;

    let discount_cents = if cmd.coupon_type == "percentage" {
        (state.subtotal_cents * cmd.value) / 100
    } else {
        // fixed
        cmd.value
    };

    let event = CouponApplied {
        coupon_code: cmd.code.clone(),
        coupon_type: cmd.coupon_type.clone(),
        value: cmd.value,
        discount_cents,
        applied_at: Some(now()),
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.CouponApplied",
        event,
    ))
}
